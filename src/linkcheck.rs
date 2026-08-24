//! Link integrity checking for a folder of Markdown notes.
//!
//! [`Exporter::check`] walks the same set of files an export would process
//! and verifies every link found in each Markdown note:
//!
//! - Obsidian references (`[[note]]`, `[[note#section]]`, `[[note#^block]]`,
//!   plus their embed forms) resolve the same way the exporter resolves them;
//! - standard Markdown links and images (`[text](target)`) must point to a
//!   file inside the checked root, and their `#anchor` fragment (when the
//!   target is Markdown) must match a heading or block id in that file;
//! - the checked root is the export boundary: a link that escapes the root
//!   (`../sibling/…`, absolute paths the vault index cannot resolve, other
//!   drives) is reported as broken even when the file exists on disk,
//!   because it will not be part of the export;
//! - external URLs (`https://…`, `mailto:…`) are skipped: reachability is a
//!   property of the network, not of the vault.
//!
//! Every link yields one [`LinkCheckReport`] with the source file, line
//! number and raw link text, so callers can render a per-link report.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use rayon::prelude::*;
use snafu::ResultExt;
use unicode_normalization::UnicodeNormalization;

use crate::references::ObsidianNoteReference;
use crate::{
    aggregate_inline_text, collapsed_ref_display, format_anchor, normalize_lexically,
    vault_contents, ExportError, Exporter, RawNoteRef, VaultIndex,
};

/// What kind of link a [`LinkCheckReport`] describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LinkKind {
    /// An Obsidian reference link, e.g. `[[note#section]]`.
    WikiLink,
    /// An Obsidian reference embed, e.g. `![[note#section]]`.
    WikiEmbed,
    /// A standard Markdown link, e.g. `[text](target.md#anchor)`.
    MarkdownLink,
    /// A standard Markdown image, e.g. `![alt](image.png)`.
    MarkdownImage,
}

/// The verdict for one checked link.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkCheckStatus {
    /// The link resolves to an existing target with a matching anchor.
    Ok,
    /// The target file was not found inside the checked root.
    MissingFile {
        /// The unresolved target as written in the link.
        target: String,
    },
    /// The target resolves outside the checked root (the export boundary):
    /// reported even when the file exists on disk.
    OutOfBounds {
        /// The escaped target as written in the link.
        target: String,
    },
    /// The target file exists but has no heading matching the referenced
    /// section (compared with the exporter's own Obsidian-style matching).
    MissingSection {
        /// The target file that was searched.
        target: String,
        /// The section that was not found.
        section: String,
    },
    /// The target file exists but contains no block with the referenced id.
    MissingBlock {
        /// The target file that was searched.
        target: String,
        /// The block id that was not found.
        block: String,
    },
    /// The source (or target) file could not be read or parsed; checkers
    /// keep going so one bad file does not hide the rest of the report.
    FileUnreadable {
        /// Why the file could not be processed.
        message: String,
    },
    /// An external URL (`https://…`, `mailto:…`): not checked, since
    /// reachability depends on the network rather than the vault.
    ExternalSkipped {
        /// The URL that was skipped.
        url: String,
    },
}

impl LinkCheckStatus {
    /// Whether this verdict means the link is broken (as opposed to healthy
    /// or deliberately not checked).
    #[must_use]
    pub const fn is_broken(&self) -> bool {
        !matches!(self, Self::Ok | Self::ExternalSkipped { .. })
    }
}

/// One checked link: where it lives, what it says, and how it fared.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LinkCheckReport {
    /// The file containing the link, relative to the checked root.
    pub source: PathBuf,
    /// 1-based line number of the link inside the source file.
    pub line: usize,
    /// The link target as written in the note (reference text or URL).
    pub raw: String,
    /// Which syntax produced the link.
    pub kind: LinkKind,
    /// The verdict.
    pub status: LinkCheckStatus,
}

/// The result of [`Exporter::check`]: every link report plus counters.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct CheckSummary {
    /// How many Markdown files were checked.
    pub files_checked: usize,
    /// One report per link found, grouped per file, files and links within a
    /// file both in sorted order.
    pub reports: Vec<LinkCheckReport>,
}

impl CheckSummary {
    /// How many links were found in total (checked and skipped).
    #[must_use]
    pub fn total_links(&self) -> usize {
        self.reports.len()
    }

    /// How many links are broken (missing target/section/block, out of
    /// bounds, or unreadable files).
    #[must_use]
    pub fn broken_links(&self) -> usize {
        self.reports.iter().filter(|r| r.status.is_broken()).count()
    }

    /// How many links were skipped because they are external URLs.
    #[must_use]
    pub fn skipped_links(&self) -> usize {
        self.reports
            .iter()
            .filter(|r| matches!(r.status, LinkCheckStatus::ExternalSkipped { .. }))
            .count()
    }
}

impl std::fmt::Display for LinkCheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::MissingFile { target } => write!(f, "broken: file not found '{target}'"),
            Self::OutOfBounds { target } => {
                write!(f, "broken: escapes the checked root '{target}'")
            }
            Self::MissingSection { target, section } => {
                write!(f, "broken: section '{section}' not found in '{target}'")
            }
            Self::MissingBlock { target, block } => {
                write!(f, "broken: block '{block}' not found in '{target}'")
            }
            Self::FileUnreadable { message } => write!(f, "unreadable: {message}"),
            Self::ExternalSkipped { url } => write!(f, "skipped: external '{url}'"),
        }
    }
}

/// Byte offsets where each line of a note starts; converts a byte offset
/// into a 1-based line number.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.match_indices('\n')
                .map(|(offset, _)| offset.saturating_add(1)),
        );
        Self { starts }
    }

    fn line_of(&self, offset: usize) -> usize {
        // The last line start at or before `offset` is the line we are on.
        let idx = self.starts.partition_point(|&start| start <= offset);
        idx.max(1)
    }
}

/// Aggregate the text of every heading in `events`, in document order —
/// the same aggregation `reduce_to_section` performs when matching section
/// references (formatting dropped, inline code/math kept, collapsed
/// wikilinks counted by their display text).
fn headings_of(events: &[Event<'_>]) -> Vec<String> {
    let mut headings = Vec::new();
    let mut in_heading = false;
    let mut heading_text = String::new();

    let mut i = 0;
    while let Some(event) = events.get(i) {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if in_heading {
                    in_heading = false;
                    headings.push(std::mem::take(&mut heading_text));
                }
            }
            _ if in_heading => match event {
                Event::Text(text) => {
                    if let Some(display) = collapsed_ref_display(events, i, text) {
                        heading_text.push_str(&display);
                        i = i.saturating_add(4);
                    } else {
                        heading_text.push_str(text);
                    }
                }
                Event::Code(text) | Event::InlineMath(text) => heading_text.push_str(text),
                Event::SoftBreak | Event::HardBreak => heading_text.push(' '),
                _ => {}
            },
            _ => {}
        }
        i = i.saturating_add(1);
    }
    headings
}

/// The normalized form headings and section queries are compared with:
/// NFC, then lowercased.
fn normalized_heading(text: &str) -> String {
    text.nfc().collect::<String>().to_lowercase()
}

/// Heading-derived anchors of one target note: the normalized heading names
/// used by Obsidian-style section matching, and the GitHub-style slugs used
/// by standard Markdown link fragments. Unreadable/unparsable targets are
/// cached too (negative caching) with `unreadable` set, so a broken note
/// costs one parse and is reported as such rather than as a missing
/// section.
#[derive(Default)]
struct TargetInfo {
    headings: HashSet<String>,
    anchors: HashSet<String>,
    unreadable: Option<String>,
}

impl TargetInfo {
    fn collect(path: &Path) -> Self {
        let (_frontmatter, events) = match Exporter::parse_raw_note(path) {
            Ok(parsed) => parsed,
            Err(error) => {
                return Self {
                    unreadable: Some(error.to_string()),
                    ..Self::default()
                }
            }
        };
        let mut info = Self::default();
        for heading in headings_of(&events) {
            info.headings.insert(normalized_heading(&heading));
            info.anchors.insert(format_anchor(&heading));
        }
        info
    }
}

/// Cache of [`TargetInfo`] shared across the parallel per-file checks.
type TargetCache = Mutex<HashMap<PathBuf, Arc<TargetInfo>>>;

fn cached_target_info(path: &Path, cache: &TargetCache) -> Arc<TargetInfo> {
    if let Some(hit) = cache.lock().expect("target cache poisoned").get(path) {
        return Arc::clone(hit);
    }
    let info = Arc::new(TargetInfo::collect(path));
    cache
        .lock()
        .expect("target cache poisoned")
        .insert(path.to_path_buf(), Arc::clone(&info));
    info
}

/// The outcome of resolving a reference's file part against the vault.
enum FileResolution<'a> {
    Found(&'a PathBuf),
    Missing,
    OutOfBounds,
}

/// Resolve the file part of a reference the way the exporter does, but keep
/// the "escaped the root" outcome distinct from "not found": the suffix
/// index only ever holds files inside the root, so index hits can never be
/// out of bounds, while references with explicit relative components are
/// resolved lexically and may climb out.
fn resolve_for_check<'a>(
    file: &str,
    source: &Path,
    root: &Path,
    index: &'a VaultIndex,
) -> FileResolution<'a> {
    if let Some(found) = index.lookup(file) {
        return FileResolution::Found(found);
    }
    // Absolute paths (e.g. `C:\x` or `/x`) point outside the checked root
    // by definition; the vault suffix index can never contain them.
    if Path::new(file).is_absolute() || file.starts_with('/') || file.starts_with('\\') {
        return FileResolution::OutOfBounds;
    }
    let has_relative_marker = file
        .split(['/', '\\'])
        .any(|component| component == "." || component == "..");
    if !has_relative_marker {
        return FileResolution::Missing;
    }
    let Some(base) = source.parent() else {
        return FileResolution::Missing;
    };
    let resolved = normalize_lexically(&base.join(file));
    if !resolved.starts_with(root) {
        return FileResolution::OutOfBounds;
    }
    index
        .lookup(&resolved.to_string_lossy())
        .map_or(FileResolution::Missing, FileResolution::Found)
}

/// Decode `%XX` escapes (as produced by `encode_link_destination` when the
/// exporter generates links); invalid escapes are kept verbatim.
fn percent_decode(input: &str) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            bytes.extend(ch.to_string().as_bytes());
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 2 {
            match chars.peek() {
                Some(next) if next.is_ascii_hexdigit() => {
                    hex.push(*next);
                    chars.next();
                }
                _ => break,
            }
        }
        if hex.len() == 2 {
            let value = u8::from_str_radix(&hex, 16).expect("two hex digits were validated");
            bytes.push(value);
        } else {
            bytes.extend(format!("%{hex}").as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Whether a link destination looks like `scheme:rest` (an external URL)
/// rather than a path. A single-letter scheme is a Windows drive (`C:\…`)
/// and counts as a path.
fn looks_like_url(dest: &str) -> bool {
    let Some((scheme, _rest)) = dest.split_once(':') else {
        return false;
    };
    if scheme.is_empty() || scheme.chars().count() == 1 {
        return false;
    }
    scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// The file part of a path, with `/`-separators normalized, for reporting.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A path relative to the checked root for reporting; paths outside the
/// root (rare: unreadable-file placeholders) fall back to the full path.
fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map_or_else(|_| display_path(path), display_path)
}

/// Whether `path` refers to the same file as one of the walked vault
/// contents (compared as exact `/`-separated relative paths: case must
/// match, matching how the exported tree is laid out).
fn known_file(root: &Path, path: &Path, known: &HashSet<String>) -> bool {
    path.strip_prefix(root)
        .map(|rel| known.contains(&display_path(rel)))
        .unwrap_or(false)
}

/// Verify an Obsidian reference (wikilink or embed) found in `source`.
fn check_obsidian_ref(
    raw: &RawNoteRef,
    events: &[Event<'_>],
    source: &Path,
    root: &Path,
    index: &VaultIndex,
    lines: &LineIndex,
    cache: &TargetCache,
) -> LinkCheckReport {
    let reference = ObsidianNoteReference::from_str(&raw.text);
    let kind = if raw.embed {
        LinkKind::WikiEmbed
    } else {
        LinkKind::WikiLink
    };
    let make = |status| LinkCheckReport {
        source: display_rel(root, source).into(),
        line: lines.line_of(raw.start),
        raw: raw.text.clone(),
        kind,
        status,
    };

    // References with no file part point at a section of the current note.
    let target: PathBuf = match reference.file {
        Some(file) => match resolve_for_check(file, source, root, index) {
            FileResolution::Found(path) => path.clone(),
            FileResolution::Missing => {
                return make(LinkCheckStatus::MissingFile {
                    target: file.to_owned(),
                })
            }
            FileResolution::OutOfBounds => {
                return make(LinkCheckStatus::OutOfBounds {
                    target: file.to_owned(),
                })
            }
        },
        None => source.to_path_buf(),
    };
    let Some(section) = reference.section else {
        return make(LinkCheckStatus::Ok);
    };

    if let Some(block) = section.strip_prefix('^') {
        // Block ids are verified against the note's own event stream so the
        // code-block exclusion and trailing/standalone rules match the
        // exporter exactly.
        let found = if target == source {
            crate::reduce_to_block(events, block).is_some()
        } else {
            match Exporter::parse_raw_note(&target) {
                Ok((_frontmatter, target_events)) => {
                    crate::reduce_to_block(&target_events, block).is_some()
                }
                Err(error) => {
                    return make(LinkCheckStatus::FileUnreadable {
                        message: error.to_string(),
                    })
                }
            }
        };
        return if found {
            make(LinkCheckStatus::Ok)
        } else {
            make(LinkCheckStatus::MissingBlock {
                target: display_rel(root, &target),
                block: block.to_owned(),
            })
        };
    }

    // Section matching reuses the exporter's semantics: the aggregate of the
    // reference text, Unicode-normalized and lowercased, must equal some
    // heading of the target. Same-file targets can use their own events.
    let normalized = normalized_heading(&aggregate_inline_text(section));
    let found = if target == source {
        headings_of(events)
            .iter()
            .any(|h| normalized_heading(h) == normalized)
    } else {
        let info = cached_target_info(&target, cache);
        if let Some(message) = &info.unreadable {
            return make(LinkCheckStatus::FileUnreadable {
                message: message.clone(),
            });
        }
        info.headings.contains(&normalized)
    };
    if found {
        make(LinkCheckStatus::Ok)
    } else {
        make(LinkCheckStatus::MissingSection {
            target: display_rel(root, &target),
            section: section.to_owned(),
        })
    }
}

/// Verify a standard Markdown link or image destination found in `source`.
/// `events` is the already-parsed event stream of `source`, reused for
/// same-file fragment targets instead of re-reading the note.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn check_markdown_dest(
    dest: &str,
    image: bool,
    events: &[Event<'_>],
    source: &Path,
    root: &Path,
    known: &HashSet<String>,
    line: usize,
    cache: &TargetCache,
) -> LinkCheckReport {
    let kind = if image {
        LinkKind::MarkdownImage
    } else {
        LinkKind::MarkdownLink
    };
    let make = |status| LinkCheckReport {
        source: display_rel(root, source).into(),
        line,
        raw: dest.to_owned(),
        kind,
        status,
    };

    let dest = percent_decode(dest).replace('\\', "/");
    if looks_like_url(&dest) {
        return make(LinkCheckStatus::ExternalSkipped { url: dest });
    }
    let (path_part, fragment) = match dest.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (dest.as_str(), None),
    };

    // A pure fragment points into the current file; otherwise resolve the
    // path relative to the note's directory.
    let target: Option<PathBuf> = if path_part.is_empty() {
        Some(source.to_path_buf())
    } else {
        let as_path = Path::new(path_part);
        if as_path.is_absolute() || path_part.starts_with('/') {
            return make(LinkCheckStatus::OutOfBounds {
                target: path_part.to_owned(),
            });
        }
        let Some(base) = source.parent() else {
            return make(LinkCheckStatus::MissingFile {
                target: path_part.to_owned(),
            });
        };
        let resolved = normalize_lexically(&base.join(path_part));
        if !resolved.starts_with(root) {
            return make(LinkCheckStatus::OutOfBounds {
                target: path_part.to_owned(),
            });
        }
        if !known_file(root, &resolved, known) {
            return make(LinkCheckStatus::MissingFile {
                target: path_part.to_owned(),
            });
        }
        Some(resolved)
    };
    let Some(target) = target else {
        return make(LinkCheckStatus::Ok);
    };

    let Some(fragment) = fragment.filter(|f| !f.is_empty()) else {
        return make(LinkCheckStatus::Ok);
    };
    // Fragments only mean something on Markdown targets; images and other
    // files have no headings to match.
    if target.extension().map(std::ffi::OsStr::to_ascii_lowercase) != Some("md".into()) {
        return make(LinkCheckStatus::Ok);
    }

    if let Some(block) = fragment.strip_prefix('^') {
        let found = if target == source {
            crate::reduce_to_block(events, block).is_some()
        } else {
            match Exporter::parse_raw_note(&target) {
                Ok((_frontmatter, target_events)) => {
                    crate::reduce_to_block(&target_events, block).is_some()
                }
                Err(error) => {
                    return make(LinkCheckStatus::FileUnreadable {
                        message: error.to_string(),
                    })
                }
            }
        };
        return if found {
            make(LinkCheckStatus::Ok)
        } else {
            make(LinkCheckStatus::MissingBlock {
                target: display_rel(root, &target),
                block: block.to_owned(),
            })
        };
    }

    // Accept both the GitHub-style slug (what the exporter generates and
    // what GitHub/VS Code navigate by) and the exact normalized heading
    // text (for hand-written links that quote the heading verbatim).
    let slug = format_anchor(fragment);
    let normalized = normalized_heading(fragment);
    let found = if target == source {
        headings_of(events)
            .iter()
            .any(|h| format_anchor(h) == slug || normalized_heading(h) == normalized)
    } else {
        let info = cached_target_info(&target, cache);
        if let Some(message) = &info.unreadable {
            return make(LinkCheckStatus::FileUnreadable {
                message: message.clone(),
            });
        }
        info.anchors.contains(&slug) || info.headings.contains(&normalized)
    };
    if found {
        make(LinkCheckStatus::Ok)
    } else {
        make(LinkCheckStatus::MissingSection {
            target: display_rel(root, &target),
            section: fragment.to_owned(),
        })
    }
}

/// Check a single note: extract Obsidian references and standard Markdown
/// links, verify each, and return one report per link sorted by line.
fn check_file(
    source: &Path,
    root: &Path,
    index: &VaultIndex,
    known: &HashSet<String>,
    cache: &TargetCache,
) -> Vec<LinkCheckReport> {
    let content = match std::fs::read_to_string(source) {
        Ok(content) => content,
        Err(error) => {
            return vec![LinkCheckReport {
                source: display_rel(root, source).into(),
                line: 0,
                raw: String::new(),
                kind: LinkKind::WikiLink,
                status: LinkCheckStatus::FileUnreadable {
                    message: error.to_string(),
                },
            }]
        }
    };
    let lines = LineIndex::new(&content);

    let (_frontmatter, events, refs) = match Exporter::parse_raw_note_with_refs(source) {
        Ok(parsed) => parsed,
        Err(error) => {
            return vec![LinkCheckReport {
                source: display_rel(root, source).into(),
                line: 0,
                raw: String::new(),
                kind: LinkKind::WikiLink,
                status: LinkCheckStatus::FileUnreadable {
                    message: error.to_string(),
                },
            }]
        }
    };

    let mut reports: Vec<LinkCheckReport> = refs
        .iter()
        .map(|raw| check_obsidian_ref(raw, &events, source, root, index, &lines, cache))
        .collect();

    // Standard Markdown links/images are collected from a second pass with
    // the same parser flavor; wikilink regions produce no Tag::Link events,
    // and code spans/blocks never yield link events, matching export-time
    // parsing semantics.
    let parser = Parser::new_ext(&content, crate::markdown_parser_options()).into_offset_iter();
    for (event, range) in parser {
        let (dest, image) = match event {
            Event::Start(Tag::Link { dest_url, .. }) => (dest_url, false),
            Event::Start(Tag::Image { dest_url, .. }) => (dest_url, true),
            _ => continue,
        };
        reports.push(check_markdown_dest(
            dest.as_ref(),
            image,
            &events,
            source,
            root,
            known,
            lines.line_of(range.start),
            cache,
        ));
    }

    reports.sort_by_key(|report| report.line);
    reports
}

impl Exporter<'_> {
    /// Check every link in the vault without writing any files.
    ///
    /// The same files an export would process are walked (honoring
    /// `start_at`, ignore files and the configured walk options), and every
    /// Obsidian reference and standard Markdown link in each Markdown note
    /// is verified against the vault: targets must exist inside the root
    /// (the export boundary — escapes are reported even when the file
    /// exists on disk), and section/block anchors must resolve in the
    /// target note. See [`CheckSummary`] for the per-link results.
    ///
    /// # Errors
    ///
    /// Returns an error when the root or `start_at` does not exist, when
    /// `start_at` lies outside the root, when either path cannot be
    /// canonicalized, or when walking the vault fails. Unreadable
    /// individual notes are reported through the summary instead of
    /// aborting the check.
    pub fn check(&mut self) -> Result<CheckSummary, ExportError> {
        if !self.root.exists() {
            return Err(ExportError::PathDoesNotExist {
                path: self.root.clone(),
            });
        }
        if self.start_at != self.root {
            if !self.start_at.exists() {
                return Err(ExportError::PathDoesNotExist {
                    path: self.start_at.clone(),
                });
            }
            if !self.start_at.starts_with(&self.root) {
                return Err(ExportError::StartAtNotUnderRoot {
                    start_at: self.start_at.clone(),
                    root: self.root.clone(),
                });
            }
        }

        // Every path this checker compares or strips prefixes from is
        // derived from one absolute, canonical form, so roots spelled ".",
        // "./sub" or with redundant components behave identically. Without
        // this, lexically normalized link targets would be compared against
        // an un-normalized boundary and in-bounds links would be flagged as
        // escapes.
        let root = std::fs::canonicalize(&self.root).context(crate::CanonicalizeSnafu {
            path: self.root.clone(),
        })?;
        let contents = Arc::from(vault_contents(&root, self.walk_options.clone())?);
        self.vault_index = Some(VaultIndex::build(&contents));
        self.vault_contents = Some(Arc::clone(&contents));
        let index = self
            .vault_index
            .clone()
            .expect("vault_index is always built above");

        // Checking a single file uses its directory as the boundary, the
        // same way a single-file export treats the file's folder as root.
        let (boundary, files): (PathBuf, Vec<PathBuf>) = if root.is_file() {
            let parent = root.parent().map(Path::to_path_buf).unwrap_or_default();
            (parent, vec![root])
        } else {
            let start_at = if self.start_at == self.root {
                root.clone()
            } else {
                std::fs::canonicalize(&self.start_at).context(crate::CanonicalizeSnafu {
                    path: self.start_at.clone(),
                })?
            };
            let files: Vec<PathBuf> = contents
                .iter()
                .filter(|file| {
                    file.starts_with(&start_at)
                        && file.extension().map(std::ffi::OsStr::to_ascii_lowercase)
                            == Some("md".into())
                })
                .cloned()
                .collect();
            (root, files)
        };

        let known: HashSet<String> = contents
            .iter()
            .filter_map(|file| file.strip_prefix(&boundary).ok().map(display_path))
            .collect();

        let cache: TargetCache = Mutex::new(HashMap::new());
        let mut reports: Vec<Vec<LinkCheckReport>> = files
            .par_iter()
            .map(|file| check_file(file, &boundary, &index, &known, &cache))
            .collect();

        // Per-file vectors are already in walk order (sorted); flatten keeps
        // a stable, deterministic overall ordering.
        let reports = reports
            .iter_mut()
            .flat_map(std::mem::take)
            .collect::<Vec<LinkCheckReport>>();

        Ok(CheckSummary {
            files_checked: files.len(),
            reports,
        })
    }
}

// Explicit format arguments are used throughout: this crate is edition 2018,
// where implicit format captures in panic!/assert! messages do not expand.
#[cfg(test)]
#[allow(clippy::uninlined_format_args)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(root: &TempDir, rel: &str, content: &str) -> PathBuf {
        let path = root.path().join(rel);
        let parent = path.parent().expect("relative paths have a parent");
        std::fs::create_dir_all(parent).expect("create dir");
        std::fs::write(&path, content).expect("write file");
        path
    }

    fn check(root: &TempDir) -> CheckSummary {
        let mut exporter = Exporter::new(root.path().to_path_buf(), root.path().to_path_buf());
        exporter.check().expect("check should succeed")
    }

    fn report_for<'a>(summary: &'a CheckSummary, raw: &str) -> &'a LinkCheckReport {
        summary
            .reports
            .iter()
            .find(|r| r.raw == raw)
            .unwrap_or_else(|| panic!("no report for raw link {:?}: {:#?}", raw, summary))
    }

    const TARGET: &str =
        "# Title\n\nBody text. ^block-one\n\n## 总纲：三份形态，两个断口\n\nSection body.\n";

    #[test]
    fn healthy_links_pass() {
        let root = TempDir::new().unwrap();
        write(&root, "target.md", TARGET);
        write(
            &root,
            "note.md",
            concat!(
                "[[target]], [[target#总纲：三份形态，两个断口]], [[target#^block-one]], [[#Local Heading]],\n",
                "[link](target.md#总纲三份形态两个断口), [same](target.md), ![img](pixel.png),\n",
                "[outside](https://example.com/what/ever)\n",
                "\n# Local Heading\n",
            ),
        );
        write(&root, "pixel.png", "not really a png");

        let summary = check(&root);
        assert_eq!(summary.broken_links(), 0, "{:#?}", summary);
        assert_eq!(summary.skipped_links(), 1);
        // Line numbers are attributed per source line (1-based).
        assert_eq!(
            report_for(&summary, "target#总纲：三份形态，两个断口").line,
            1
        );
        assert_eq!(
            report_for(&summary, "https://example.com/what/ever").line,
            3
        );
    }

    #[test]
    fn missing_file_is_reported() {
        let root = TempDir::new().unwrap();
        write(&root, "note.md", "Gone: [[missing-note]]\n");
        let summary = check(&root);
        let report = report_for(&summary, "missing-note");
        assert_eq!(
            report.status,
            LinkCheckStatus::MissingFile {
                target: "missing-note".into()
            }
        );
        assert!(report.status.is_broken());
    }

    #[test]
    fn escaping_links_are_out_of_bounds_even_when_present() {
        let root = TempDir::new().unwrap();
        // A real file outside the checked root: it exists on disk, but the
        // root is the export boundary so the link must still be flagged.
        let outside = root.path().parent().expect("tempdir parent").join(format!(
            "outside-{}.md",
            root.path().file_name().unwrap().to_string_lossy()
        ));
        std::fs::write(&outside, "# Outside\n").expect("write outside file");
        write(
            &root,
            "note.md",
            "Wiki: [[../outside]] and markdown: [x](../outside.md) and absolute: [y](/etc/hosts)\n",
        );

        let summary = check(&root);
        assert_eq!(summary.broken_links(), 3, "{:#?}", summary);
        assert_eq!(
            report_for(&summary, "../outside").status,
            LinkCheckStatus::OutOfBounds {
                target: "../outside".into()
            }
        );
        assert!(matches!(
            report_for(&summary, "../outside.md").status,
            LinkCheckStatus::OutOfBounds { .. }
        ));
        assert!(matches!(
            report_for(&summary, "/etc/hosts").status,
            LinkCheckStatus::OutOfBounds { .. }
        ));
        std::fs::remove_file(outside).ok();
    }

    #[test]
    fn missing_section_and_block_are_reported() {
        let root = TempDir::new().unwrap();
        write(&root, "target.md", TARGET);
        write(
            &root,
            "note.md",
            "[[target#Nope]] plus [anchor](target.md#nope) and [[target#^nope]]\n",
        );
        write(
            &root,
            "self.md",
            "[[#Missing Heading]] and [frag](#missing-heading)\n",
        );

        let summary = check(&root);
        assert_eq!(summary.broken_links(), 5, "{:#?}", summary);
        for raw in [
            "target#Nope",
            "target.md#nope",
            "target#^nope",
            "#Missing Heading",
        ] {
            assert!(report_for(&summary, raw).status.is_broken(), "{}", raw);
        }
        // The hand-written markdown fragment with the same shape is broken too.
        assert!(report_for(&summary, "#missing-heading").status.is_broken());
    }

    #[test]
    fn links_inside_code_are_not_checked() {
        let root = TempDir::new().unwrap();
        write(
            &root,
            "note.md",
            concat!(
                "```markdown\n",
                "[[missing-note]] and [x](missing.md)\n",
                "```\n",
                "Inline `[[missing-note]]` code too.\n",
            ),
        );
        let summary = check(&root);
        assert_eq!(summary.total_links(), 0, "{:#?}", summary);
    }

    #[test]
    fn percent_encoded_destinations_resolve() {
        let root = TempDir::new().unwrap();
        write(&root, "target.md", TARGET);
        write(&root, "with space.md", "# Spacy\n");
        write(&root, "note.md", "[enc](with%20space.md) and [anchor](target.md#%E6%80%BB%E7%BA%B2%E4%B8%89%E4%BB%BD%E5%BD%A2%E6%80%81%E4%B8%A4%E4%B8%AA%E6%96%AD%E5%8F%A3)\n");
        let summary = check(&root);
        assert_eq!(summary.broken_links(), 0, "{:#?}", summary);
    }

    #[test]
    fn wikilink_section_matching_follows_obsidian_semantics() {
        // `[[target#总纲：三份形态，两个断口]]` must match the verbatim
        // heading (Obsidian semantics), independent of slug rules.
        let root = TempDir::new().unwrap();
        write(&root, "target.md", TARGET);
        write(&root, "note.md", "[[target#总纲：三份形态，两个断口]]\n");
        let summary = check(&root);
        assert_eq!(summary.broken_links(), 0, "{:#?}", summary);
    }

    #[test]
    fn oddly_spelled_roots_classify_links_correctly() {
        // Roots spelled with redundant `.`/`..` components must behave like
        // their canonical form: in-bounds links stay ok, missing files are
        // "not found" (not "escapes"), real escapes are still flagged.
        let root = TempDir::new().unwrap();
        write(&root, "target.md", TARGET);
        write(
            &root,
            "sub/note.md",
            "ok: [[../target]] plus [t](../target.md), missing: [[gone]]
",
        );
        let weird = root.path().join("./sub/../.");

        let mut exporter = Exporter::new(weird, root.path().to_path_buf());
        let summary = exporter.check().expect("check should succeed");
        assert_eq!(
            report_for(&summary, "../target").status,
            LinkCheckStatus::Ok,
            "in-bounds reference from a subdir, weird root spelling"
        );
        assert!(
            matches!(
                report_for(&summary, "../target.md").status,
                LinkCheckStatus::Ok
            ),
            "in-bounds markdown link, weird root spelling"
        );
        assert!(
            matches!(
                report_for(&summary, "gone").status,
                LinkCheckStatus::MissingFile { .. }
            ),
            "missing file must not be misclassified as an escape"
        );
    }

    #[test]
    fn absolute_wikilink_targets_are_out_of_bounds() {
        let root = TempDir::new().unwrap();
        write(
            &root,
            "note.md",
            "[[/abs/path]] and windows [[C:\\other\\note]]\n",
        );
        let summary = check(&root);
        assert!(matches!(
            report_for(&summary, "/abs/path").status,
            LinkCheckStatus::OutOfBounds { .. }
        ));
        assert!(matches!(
            report_for(&summary, "C:\\other\\note").status,
            LinkCheckStatus::OutOfBounds { .. }
        ));
    }

    #[test]
    fn unreadable_target_reports_file_unreadable_not_missing_section() {
        // A section link into a note whose frontmatter is broken must say
        // the target is unreadable, not send the user hunting for a heading
        // that may well exist.
        let root = TempDir::new().unwrap();
        write(
            &root,
            "broken.md",
            "---
not: [valid: yaml
---

# Heading
",
        );
        write(
            &root,
            "note.md",
            "[[broken#Heading]]
",
        );
        let summary = check(&root);
        assert!(
            matches!(
                report_for(&summary, "broken#Heading").status,
                LinkCheckStatus::FileUnreadable { .. }
            ),
            "{:#?}",
            summary
        );
    }

    #[test]
    fn unreadable_frontmatter_is_reported_per_file() {
        let root = TempDir::new().unwrap();
        write(&root, "note.md", "ok: [[note]]\n");
        write(
            &root,
            "broken.md",
            "---\nnot: [valid: yaml\n---\n\n[[note]]\n",
        );
        let summary = check(&root);
        assert_eq!(summary.files_checked, 2);
        let unreadable = summary
            .reports
            .iter()
            .filter(|r| matches!(r.status, LinkCheckStatus::FileUnreadable { .. }))
            .count();
        assert_eq!(unreadable, 1, "{:#?}", summary);
    }
}
