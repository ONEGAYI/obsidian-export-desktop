//! Excalidraw drawing files: detection, scene extraction and prescan-time
//! conversion into image assets.
//!
//! The Obsidian Excalidraw plugin stores drawings in one of three shapes:
//! markdown whose `## Drawing` section carries the scene JSON in a
//! ```` ```compressed-json ```` fence (LZ-String `compressToBase64`, chunked
//! into 256-char lines separated by blank lines), the same layout with a
//! plain ```` ```json ```` fence in older files, or bare scene JSON in legacy
//! `.excalidraw` files. Enabled through `--render-diagrams excalidraw`, the
//! export prescan converts every drawing under the export scope into an
//! image placed next to the source file's output-tree position (matching the
//! plugin's Auto-Export naming, e.g. `x.excalidraw.md` → `x.excalidraw.svg`)
//! and records the outcome in an [`ExcalidrawIndex`] so the export pass can
//! swap embeds and links for image references, or degrade them to plain
//! links when conversion failed. LaTeX formulas and pasted images travel
//! inside the scene as data URLs and need no extra tooling.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;
use snafu::{ResultExt, Snafu};
use tempfile::TempDir;

use crate::diagrams::{self, DiagramFormat, DiagramState};
use crate::ExportEvent;

// ---------------------------------------------------------------------------
// LZ-String decompression
// ---------------------------------------------------------------------------

#[derive(Debug, Snafu)]
pub enum DecompressError {
    #[snafu(display("compressed data is empty"))]
    Empty,

    #[snafu(display("invalid base64 character {byte:#x} at index {index}"))]
    InvalidChar { byte: u8, index: usize },

    #[snafu(display("compressed stream ended unexpectedly"))]
    UnexpectedEof,

    #[snafu(display("invalid dictionary reference {code}"))]
    InvalidCode { code: u64 },

    #[snafu(display("invalid 2-bit stream header {code}"))]
    InvalidHeader { code: u64 },

    #[snafu(display("decompressed data is not valid text: {source}"))]
    InvalidUtf16 { source: std::string::FromUtf16Error },
}

#[allow(clippy::arithmetic_side_effects)]
fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// LZ-String bit reader over base64 characters: six bits per character,
/// consumed most-significant first (the reader's mask starts at 32 and halves
/// until it wraps, at which point the next character is fetched).
struct Base64BitReader<'a> {
    chars: &'a [u8],
    index: usize,
    value: u32,
    position: u32,
}

impl<'a> Base64BitReader<'a> {
    fn new(chars: &'a [u8]) -> Option<Self> {
        let first = *chars.first()?;
        Some(Self {
            chars,
            index: 1,
            value: base64_value(first)?,
            position: 32,
        })
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn advance(&mut self) -> Result<(), DecompressError> {
        let byte = *self
            .chars
            .get(self.index)
            .ok_or(DecompressError::UnexpectedEof)?;
        self.value = base64_value(byte).ok_or(DecompressError::InvalidChar {
            byte,
            index: self.index,
        })?;
        self.index += 1;
        Ok(())
    }

    /// Read `count` bits, least-significant bit first (LZ-String order).
    #[allow(clippy::arithmetic_side_effects)]
    fn read_bits(&mut self, count: u32) -> Result<u64, DecompressError> {
        let mut bits: u64 = 0;
        let mut power: u64 = 1;
        for _ in 0..count {
            if self.value & self.position > 0 {
                bits += power;
            }
            self.position >>= 1_u32;
            if self.position == 0 {
                self.position = 32;
                self.advance()?;
            }
            power <<= 1_u64;
        }
        Ok(bits)
    }
}

/// Decompress an LZ-String `compressToBase64` payload, as written by the
/// Obsidian Excalidraw plugin (newlines between chunks and trailing `=`
/// padding are tolerated). Ported from the reference JavaScript
/// implementation's `_decompress`; the format is frozen so the port needs no
/// upstream tracking.
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
pub fn decompress_from_base64(input: &str) -> Result<String, DecompressError> {
    // The plugin chunks the payload into 256-char lines separated by blank
    // lines; none of that whitespace is part of the compressed data.
    let cleaned: String = input
        .chars()
        .filter(|c| !matches!(c, '\n' | '\r'))
        .collect();
    let cleaned = cleaned.trim_end_matches('=');
    let bytes = cleaned.as_bytes();
    let mut reader = Base64BitReader::new(bytes).ok_or(DecompressError::Empty)?;

    // Stream header: two bits deciding the width of the first literal.
    let first = match reader.read_bits(2)? {
        0 => u16::try_from(reader.read_bits(8)?).expect("8 bits fit in u16"),
        1 => u16::try_from(reader.read_bits(16)?).expect("16 bits fit in u16"),
        2 => return Ok(String::new()),
        code => return Err(DecompressError::InvalidHeader { code }),
    };

    // Dictionary slots 0..=2 exist in the reference implementation but are
    // never read; the first literal occupies slot 3 without touching the
    // enlarge counter.
    let mut dictionary: Vec<Vec<u16>> = vec![Vec::new(), Vec::new(), Vec::new(), vec![first]];
    let mut result: Vec<u16> = vec![first];
    let mut enlarge_in: u64 = 4;
    let mut num_bits: u32 = 3;
    let mut last: (usize, usize) = (0, 1);

    loop {
        let before = result.len();
        let code = reader.read_bits(num_bits)?;
        let slice: Vec<u16> = match code {
            // Narrow/wide literal: added to the dictionary inside the arm,
            // mirroring the reference implementation's per-case bookkeeping.
            0 | 1 => {
                let width = if code == 0 { 8 } else { 16 };
                let literal =
                    u16::try_from(reader.read_bits(width)?).expect("width bits fit in u16");
                dictionary.push(vec![literal]);
                enlarge_in -= 1;
                if enlarge_in == 0 {
                    enlarge_in = 1 << num_bits;
                    num_bits += 1;
                }
                vec![literal]
            }
            // End-of-stream marker.
            2 => break,
            // Reference to the not-yet-defined next dictionary slot: the
            // previous segment plus its own first unit.
            code if code == u64::try_from(dictionary.len()).expect("usize fits u64") => {
                let mut repeated = result[last.0..last.1].to_vec();
                let head = *repeated
                    .first()
                    .ok_or(DecompressError::InvalidCode { code })?;
                repeated.push(head);
                repeated
            }
            code if code < u64::try_from(dictionary.len()).expect("usize fits u64") => {
                let idx = usize::try_from(code).expect("code fits usize");
                dictionary[idx].clone()
            }
            code => return Err(DecompressError::InvalidCode { code }),
        };

        result.extend_from_slice(&slice);

        // Every decoded segment also adds "previous segment + this segment's
        // first unit" to the dictionary, then checks the enlarge counter.
        let head = *slice.first().expect("slice is never empty");
        let mut new_entry = result[last.0..last.1].to_vec();
        new_entry.push(head);
        dictionary.push(new_entry);
        enlarge_in -= 1;
        if enlarge_in == 0 {
            enlarge_in = 1 << num_bits;
            num_bits += 1;
        }
        last = (before, result.len());
    }

    String::from_utf16(&result).context(InvalidUtf16Snafu)
}

// ---------------------------------------------------------------------------
// Scene extraction and detection
// ---------------------------------------------------------------------------

#[derive(Debug, Snafu)]
pub enum SceneError {
    #[snafu(display("{source}"))]
    Decompress { source: DecompressError },

    #[snafu(display("no drawing section found"))]
    NoDrawing,

    #[snafu(display("extracted data is not an Excalidraw scene"))]
    NotAScene,

    #[snafu(display("scene is not valid JSON: {source}"))]
    InvalidJson { source: serde_json::Error },
}

/// Fenced `compressed-json` block carrying the LZ-String payload. Matched
/// loosely (anywhere in the file): an Excalidraw file contains exactly one,
/// and false positives on ordinary notes are impossible because extraction
/// only runs on files already detected as Excalidraw.
static COMPRESSED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```compressed-json\r?\n(.*?)\r?\n```").expect("valid regex"));

/// Fenced `json` block following the `## Drawing` heading (the
/// uncompressed variant used by older plugin versions and the
/// compression-disabled setting).
static DRAWING_JSON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)## Drawing[^`]*```json\r?\n(.*?)\r?\n```").expect("valid regex")
});

/// Extract the Excalidraw scene JSON from a file's text: compressed fence
/// first, then the plain `json` fence under `## Drawing`, then the whole
/// file as bare JSON (legacy `.excalidraw` shape). Whatever shape matched,
/// the result must parse as JSON with an `elements` array to count as a
/// scene.
pub fn extract_scene(text: &str) -> Result<String, SceneError> {
    let scene = if let Some(captures) = COMPRESSED_RE.captures(text) {
        let compressed = captures.get(1).map_or("", |m| m.as_str());
        decompress_from_base64(compressed).context(DecompressSnafu)?
    } else if let Some(captures) = DRAWING_JSON_RE.captures(text) {
        captures.get(1).map_or("", |m| m.as_str()).to_owned()
    } else {
        // Legacy `.excalidraw` files may carry a UTF-8 BOM; strip it before
        // both the shape check and the JSON handoff (serde_json rejects it).
        let bare = text.trim_start_matches('\u{FEFF}').trim_start();
        if !bare.starts_with('{') {
            return Err(SceneError::NoDrawing);
        }
        bare.to_owned()
    };

    let value: Value = serde_json::from_str(&scene).context(InvalidJsonSnafu)?;
    if !value.get("elements").is_some_and(Value::is_array) {
        return Err(SceneError::NotAScene);
    }
    Ok(scene)
}

/// Whether the path's own name marks it as an Excalidraw file (legacy
/// `.excalidraw`, or the plugin's default `.excalidraw.md`).
pub fn is_excalidraw_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|name| name.ends_with(".excalidraw") || name.ends_with(".excalidraw.md"))
}

/// Whether a markdown file's frontmatter carries the `excalidraw-plugin`
/// key (the plugin's `.md`-extension shape, used for Logseq compatibility).
/// Matching is line-anchored inside the frontmatter block only, so ordinary
/// notes mentioning the key in prose or in body frontmatter-like text don't
/// match.
pub fn markdown_has_excalidraw_frontmatter(text: &str) -> bool {
    let mut lines = text.lines();
    match lines.next() {
        Some(first) if first.trim_end() == "---" => (),
        _ => return false,
    }
    for line in lines {
        if line.trim_end() == "---" {
            return false;
        }
        if line.trim_start().starts_with("excalidraw-plugin:") {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Conversion index
// ---------------------------------------------------------------------------

/// Outcome of the prescan conversion for one Excalidraw file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcalidrawEntry {
    /// Converted successfully; the asset sits at the source path with its
    /// extension replaced by the run's diagram format.
    Converted,
    /// Conversion failed (or the file sits outside the export scope); the
    /// original file is not exported and references degrade to plain links.
    Failed,
}

/// Prescan-time map from Excalidraw source paths to conversion outcomes,
/// shared immutably across the parallel export pass. For [`ExcalidrawEntry::Converted`]
/// the asset location is derived on demand as `src.with_extension(format)`,
/// keeping the index in lockstep with the plugin's Auto-Export naming.
#[derive(Debug)]
pub struct ExcalidrawIndex {
    format: DiagramFormat,
    entries: HashMap<PathBuf, ExcalidrawEntry>,
}

impl ExcalidrawIndex {
    fn new(format: DiagramFormat) -> Self {
        Self {
            format,
            entries: HashMap::new(),
        }
    }

    pub(crate) fn get(&self, src: &Path) -> Option<ExcalidrawEntry> {
        self.entries.get(src).copied()
    }

    /// The vault-side path whose output-tree mirror holds the converted
    /// asset: `None` unless the source converted successfully.
    pub(crate) fn asset_path(&self, src: &Path) -> Option<PathBuf> {
        (self.get(src) == Some(ExcalidrawEntry::Converted))
            .then(|| src.with_extension(self.format.as_str()))
    }

    /// Whether `path` is a registered drawing source, or one of its
    /// rendered-asset twins (the same path with a `.svg`/`.png` extension —
    /// both formats are checked regardless of the run's format, matching the
    /// plugin's Auto-Export outputs). The export pass skips both: the source
    /// is replaced by its asset (or nothing, on failure), and a stale twin in
    /// the vault must not be copied over the freshly rendered asset at the
    /// identical output-tree position.
    pub(crate) fn covers(&self, path: &Path) -> bool {
        if self.entries.contains_key(path) {
            return true;
        }
        self.entries
            .keys()
            .any(|src| src.with_extension("svg") == path || src.with_extension("png") == path)
    }
}

// ---------------------------------------------------------------------------
// Prescan conversion
// ---------------------------------------------------------------------------

#[derive(Debug, Snafu)]
enum ConvertError {
    #[snafu(display("failed to read file: {source}"))]
    Read { source: std::io::Error },

    #[snafu(display("{source}"))]
    Scene { source: SceneError },

    #[snafu(display("{source}"))]
    Render {
        source: diagrams::DiagramRenderError,
    },

    #[snafu(display("failed to write asset: {source}"))]
    Write { source: std::io::Error },
}

/// Convert every detected Excalidraw file and return the outcome index.
///
/// Runs inside the export prescan, serially, before any output file is
/// written: the parallel export pass then sees a settled index, so a file
/// whose conversion failed is known to degrade its references rather than
/// leaving image links to an asset that never lands. Each file reports
/// `DiagramRender` progress and failures emit a warning carrying the source
/// path; both keep going so one broken drawing doesn't sink the export.
pub fn convert_all(
    files: &[PathBuf],
    state: &DiagramState,
    destination: &Path,
    start_at: &Path,
    on_event: &dyn Fn(&ExportEvent),
    on_warning: &dyn Fn(&Path, String),
) -> ExcalidrawIndex {
    let format = state.format();
    let mut index = ExcalidrawIndex::new(format);

    for file in files {
        let (slot, total) = state.claim_render_slot();
        on_event(&ExportEvent::DiagramRender {
            language: diagrams::EXCALIDRAW_LANGUAGE.to_owned(),
            index: slot,
            total,
        });

        match convert_one(file, state, destination, start_at, format) {
            Ok(()) => {
                index
                    .entries
                    .insert(file.clone(), ExcalidrawEntry::Converted);
            }
            Err(error) => {
                on_warning(
                    file,
                    format!(
                        "failed to render Excalidraw drawing '{}': {error}; the file is not \
                         exported and references to it degrade to plain links",
                        file.display()
                    ),
                );
                index.entries.insert(file.clone(), ExcalidrawEntry::Failed);
            }
        }
    }
    index
}

/// Convert one drawing: extract the scene, render through the external tool
/// into a temporary file, then copy the asset into its output-tree position.
fn convert_one(
    file: &Path,
    state: &DiagramState,
    destination: &Path,
    start_at: &Path,
    format: DiagramFormat,
) -> Result<(), ConvertError> {
    let text = fs::read_to_string(file).context(ReadSnafu)?;
    let scene = extract_scene(&text).context(SceneSnafu)?;

    let workdir = TempDir::new().context(WriteSnafu)?;
    let tmp_out = workdir
        .path()
        .join(format!("drawing-out.{}", format.as_str()));
    diagrams::render_excalidraw_scene(state, &scene, format, &tmp_out).context(RenderSnafu)?;

    // Output-tree position of the asset. Directory mode mirrors the source's
    // vault-relative path. Single-file mode (start_at IS the drawing, so the
    // strip leaves an empty path — `dest.join("")` would otherwise graft the
    // extension onto the directory itself) derives the position from the
    // source file name the same way `run()` places the exported file: inside
    // a directory destination, beside a file destination.
    let relative = file
        .strip_prefix(start_at)
        .expect("prescan only collects files under start_at");
    let asset = if relative.as_os_str().is_empty() {
        let name = file
            .file_name()
            .expect("a start_at file always has a file name");
        let base = if destination.is_dir() {
            destination.to_path_buf()
        } else {
            destination
                .parent()
                .expect("a file destination always has a parent")
                .to_path_buf()
        };
        base.join(name).with_extension(format.as_str())
    } else {
        destination.join(relative).with_extension(format.as_str())
    };
    if let Some(parent) = asset.parent() {
        fs::create_dir_all(parent).context(WriteSnafu)?;
    }
    fs::copy(&tmp_out, &asset).context(WriteSnafu)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Vectors produced with the reference lz-string npm package
    // (`LZString.compressToBase64`), including the shapes the Excalidraw
    // plugin actually writes: chunked whitespace (stripped here) and `=`
    // padding.
    const ROUNDTRIP_VECTORS: &[(&str, &str)] = &[
        ("hello world", "BYUwNmD2AEDukCcwBMg="),
        ("", "Q==="),
        ("{\"a\":1}", "N4IghiBcCMC+Q==="),
        (
            "中文测试：同步与异步毛刺 ✏️ emoji",
            "rRynDTTStoq9GFj/gwKoptaDg5QQPp0NvWguJQASHhyQ8H+YFMBbAewCsBLIA===",
        ),
        (
            "{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{\"id\":\"x\",\"type\":\"text\",\
             \"text\":\"Golden\"}],\"appState\":{\"viewBackgroundColor\":\"#ffffff\"},\"files\":{}}",
            "N4IgLgngDgpiBcIYA8DGBDANgSwCYCd0B3EAGhADcZ8BnbAewDsEAmcmTGAWxkbBoQBtUHgQhkZcNDiIwKMJ\
             LnIFiAOL1MuXiAC+AXXLooUAMph0chKArYYRAELpUAawDm+egFdGuAMIb6fDEAYgAzcIjdclDsTgF4YB0dIA",
        ),
    ];

    #[test]
    fn decompresses_reference_vectors() {
        for (plain, compressed) in ROUNDTRIP_VECTORS {
            assert_eq!(
                &decompress_from_base64(compressed).expect("decompress"),
                plain
            );
        }
    }

    #[test]
    fn decompresses_highly_repetitive_input() {
        // Exercises the "reference to the not-yet-defined dictionary slot"
        // path, which only kicks in once the dictionary has absorbed repeats.
        let compressed = "IY18ZXTt/DFOS1b0c17Pd/wYUcSaWeRZVdTbXTEA";
        let plain = decompress_from_base64(compressed).expect("decompress");
        assert_eq!(plain.chars().count(), 1000);
        assert!(plain.chars().all(|c| c == 'a'));
    }

    #[test]
    fn decompress_tolerates_plugin_chunking_and_padding() {
        // The plugin breaks the payload into 256-char lines separated by
        // blank lines; the decompressor must ignore all of that whitespace.
        let chunked = "BYUwNmD2AEDukCc\n\nwBMg=\n\n=";
        assert_eq!(
            decompress_from_base64(chunked).expect("decompress"),
            "hello world"
        );
    }

    #[test]
    fn decompress_rejects_garbage() {
        // Invalid base64 character in the very first position.
        decompress_from_base64("!!!!").unwrap_err();
        // A stream cut off mid-dictionary-reference.
        decompress_from_base64("N4KAkARALgngDgUwgLgAQQQDwMYEMA2AlgCYBO").unwrap_err();
        // Empty input has no header to read.
        decompress_from_base64("").unwrap_err();
    }

    #[test]
    fn extracts_scene_from_compressed_fence() {
        let (_, compressed) = ROUNDTRIP_VECTORS
            .get(4)
            .expect("vector with an excalidraw scene");
        let file = format!("---\nexcalidraw-plugin: parsed\n---\n\n%%\n## Drawing\n```compressed-json\n{compressed}\n```\n%%\n");
        let scene = extract_scene(&file).expect("extract");
        assert!(scene.starts_with("{\"type\":\"excalidraw\""));
    }

    #[test]
    fn extracts_scene_from_plain_json_fence_under_drawing() {
        let file = "---\nexcalidraw-plugin: parsed\n---\n\n%%\n## Drawing\n```json\n{\"type\":\"excalidraw\",\"elements\":[]}\n```\n%%\n";
        let scene = extract_scene(file).expect("extract");
        assert!(scene.contains("\"elements\":[]"));
    }

    #[test]
    fn extracts_scene_from_bare_json_file() {
        let scene =
            extract_scene("  {\"type\":\"excalidraw\",\"elements\":[{}]}").expect("extract");
        assert!(scene.contains("excalidraw"));
    }

    #[test]
    fn extraction_rejects_non_scene_content() {
        // No drawing section at all.
        extract_scene("# Just a note\n\nHello\n").unwrap_err();
        // A json fence not under ## Drawing, whose content is not a scene.
        extract_scene("```json\n{\"a\":1}\n```").unwrap_err();
        // Corrupted compressed payload.
        extract_scene("## Drawing\n```compressed-json\n!!!!\n```").unwrap_err();
        // A json fence under ## Drawing whose content is not JSON at all.
        assert!(extract_scene("## Drawing\n```json\nnot json at all\n```")
            .unwrap_err()
            .to_string()
            .contains("not valid JSON"));
        // Valid JSON but not a scene (no elements array): a bare-JSON file
        // carrying arbitrary data must not count as a drawing.
        assert!(extract_scene("{\"a\":1}")
            .unwrap_err()
            .to_string()
            .contains("not an Excalidraw scene"));
    }

    #[test]
    fn bare_json_tolerates_leading_bom() {
        let scene =
            extract_scene("\u{FEFF}{\"type\":\"excalidraw\",\"elements\":[]}").expect("extract");
        assert!(scene.contains("excalidraw"));
    }

    #[test]
    fn detects_excalidraw_paths() {
        assert!(is_excalidraw_path(Path::new(
            "vault/Drawing 2026.excalidraw"
        )));
        assert!(is_excalidraw_path(Path::new("vault/drawing.excalidraw.md")));
        assert!(!is_excalidraw_path(Path::new("vault/drawing.md")));
        assert!(!is_excalidraw_path(Path::new("vault/drawing.svg")));
        assert!(!is_excalidraw_path(Path::new("vault/notexcalidraw")));
    }

    #[test]
    fn detects_frontmatter_key_line_anchored() {
        assert!(markdown_has_excalidraw_frontmatter(
            "---\nexcalidraw-plugin: parsed\ntags: [excalidraw]\n---\nbody"
        ));
        // A similarly-named key must not match.
        assert!(!markdown_has_excalidraw_frontmatter(
            "---\nmy-excalidraw-plugin: raw\n---\n"
        ));
        // Key mentioned in the body, not frontmatter: no match.
        assert!(!markdown_has_excalidraw_frontmatter(
            "---\ntags: [x]\n---\nUse excalidraw-plugin: parsed here"
        ));
        // Indented key inside frontmatter still matches (YAML allows it in
        // exotic nesting; better one false positive than a missed drawing).
        assert!(markdown_has_excalidraw_frontmatter(
            "---\nfoo:\n  excalidraw-plugin: parsed\n---\n"
        ));
        // No frontmatter at all.
        assert!(!markdown_has_excalidraw_frontmatter(
            "excalidraw-plugin: parsed\n"
        ));
    }
}
