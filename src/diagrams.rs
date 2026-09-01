//! Rendering of Obsidian special diagram code blocks into image assets.
//!
//! Obsidian renders fenced code blocks such as ```` ```dot ```` or
//! ```` ```mermaid ```` through built-in support or community plugins, but
//! plain Markdown consumers show them as literal code. This module converts
//! such blocks into standalone image files by shelling out to the
//! corresponding local tools, so exports embed a regular Markdown image
//! reference instead.
//!
//! Tool discovery prefers an explicitly configured executable path and falls
//! back to a `PATH` scan. On Windows the scan honors `PATHEXT` and resolves
//! `.cmd`/`.bat` shims (as installed by npm) through `cmd.exe`, because
//! `CreateProcess` cannot execute command scripts directly.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(windows)]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::{fs, thread};

use pathdiff::diff_paths;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Parser, Tag, TagEnd};
use snafu::{ResultExt, Snafu};

use crate::{encode_link_destination, Context, ExportEvent, MarkdownEvents};

/// How long a single external tool invocation may run before it is killed.
/// Generous enough for mmdc's headless-browser startup; prevents a wedged
/// renderer from hanging the whole export.
const TOOL_TIMEOUT: Duration = Duration::from_secs(60);

/// How long the collecting side waits for a reader thread to hand over its
/// buffer after the child is gone. Pipes normally hit EOF within
/// milliseconds, but a descendant that inherited a pipe handle (e.g. a
/// browser spawned by a jammed renderer) can keep it open indefinitely, so
/// waiting must be bounded — its output is lost, the export is not.
const READER_GRACE: Duration = Duration::from_secs(5);

/// Cap on how much of a note's stem is carried over into asset filenames, so
/// that `<stem>-<16 hex>.<ext>` stays clear of Windows path-length limits.
const ASSET_STEM_MAX_CHARS: usize = 80;

/// Output image format for rendered diagrams.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagramFormat {
    /// Vector graphics; the default and the fallback for renderers without
    /// raster output.
    Svg,
    /// Raster image; supported by a subset of renderers.
    Png,
}

impl DiagramFormat {
    /// Parse a format name as accepted on the CLI (`svg`/`png`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "svg" => Some(Self::Svg),
            "png" => Some(Self::Png),
            _ => None,
        }
    }

    /// The canonical name, used in CLI flags and asset file extensions.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
        }
    }
}

/// An external renderer for a fenced diagram language.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagramRenderer {
    /// Graphviz DOT (`dot` / `graphviz` blocks), rendered with `dot`.
    Dot,
    /// Mermaid (`mermaid` / `mmd` blocks), rendered with `mmdc`.
    Mermaid,
    /// `WaveDrom` timing diagrams (`wavedrom` blocks), rendered with `wavedrom`.
    WaveDrom,
    /// `TikZ` drawings (`tikz` blocks), rendered with `latex` + `dvisvgm`.
    TikZ,
}

impl DiagramRenderer {
    /// Map a fenced code block's info-string language to a renderer.
    ///
    /// Matching is case-insensitive on the first whitespace-delimited word.
    #[must_use]
    pub fn from_language(language: &str) -> Option<Self> {
        let first = language.split_whitespace().next().unwrap_or_default();
        match first.to_lowercase().as_str() {
            "dot" | "graphviz" => Some(Self::Dot),
            "mermaid" | "mmd" => Some(Self::Mermaid),
            "wavedrom" => Some(Self::WaveDrom),
            "tikz" => Some(Self::TikZ),
            _ => None,
        }
    }

    /// Map a renderer name as accepted on the CLI and in the GUI options.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dot" => Some(Self::Dot),
            "mermaid" => Some(Self::Mermaid),
            "wavedrom" => Some(Self::WaveDrom),
            "tikz" => Some(Self::TikZ),
            _ => None,
        }
    }

    /// The canonical renderer name (CLI flags, GUI options, asset hashing).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Mermaid => "mermaid",
            Self::WaveDrom => "wavedrom",
            Self::TikZ => "tikz",
        }
    }

    /// The external tools this renderer needs to be present.
    #[must_use]
    pub const fn tool_names(self) -> &'static [ToolName] {
        match self {
            Self::Dot => &[ToolName::Dot],
            Self::Mermaid => &[ToolName::Mmdc],
            Self::WaveDrom => &[ToolName::WaveDrom],
            Self::TikZ => &[ToolName::Latex, ToolName::Dvisvgm],
        }
    }

    /// Whether the renderer can produce the requested format natively.
    #[must_use]
    pub const fn supports(self, format: DiagramFormat) -> bool {
        matches!(
            (self, format),
            (Self::Dot | Self::Mermaid, _) | (Self::WaveDrom | Self::TikZ, DiagramFormat::Svg)
        )
    }

    /// The format actually produced for a requested format; renderers without
    /// raster output fall back to SVG (the caller is expected to warn).
    #[must_use]
    pub const fn effective_format(self, requested: DiagramFormat) -> DiagramFormat {
        if self.supports(requested) {
            requested
        } else {
            DiagramFormat::Svg
        }
    }
}

/// An external executable a renderer depends on.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolName {
    Dot,
    Mmdc,
    WaveDrom,
    Latex,
    Dvisvgm,
}

impl ToolName {
    /// Parse a tool name as accepted by `--diagram-bin <TOOL>=<PATH>`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dot" => Some(Self::Dot),
            "mmdc" => Some(Self::Mmdc),
            "wavedrom" => Some(Self::WaveDrom),
            "latex" => Some(Self::Latex),
            "dvisvgm" => Some(Self::Dvisvgm),
            _ => None,
        }
    }

    /// The canonical executable name, also used for explicit-path overrides.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Mmdc => "mmdc",
            Self::WaveDrom => "wavedrom",
            Self::Latex => "latex",
            Self::Dvisvgm => "dvisvgm",
        }
    }

    /// Uppercase suffix for the debug-only test hook
    /// `OBSIDIAN_EXPORT_DIAGRAM_BIN_<TOOL>`.
    #[must_use]
    pub const fn env_key(self) -> &'static str {
        match self {
            Self::Dot => "DOT",
            Self::Mmdc => "MMDC",
            Self::WaveDrom => "WAVEDROM",
            Self::Latex => "LATEX",
            Self::Dvisvgm => "DVISVGM",
        }
    }

    /// Short human-facing hint for installing the tool.
    #[must_use]
    pub const fn install_hint(self) -> &'static str {
        match self {
            Self::Dot => "install Graphviz (https://graphviz.org/download/)",
            Self::Mmdc => "npm install -g @mermaid-js/mermaid-cli",
            Self::WaveDrom => "npm install -g wavedrom",
            Self::Latex | Self::Dvisvgm => {
                "install a TeX distribution with TikZ and dvisvgm (e.g. TeX Live)"
            }
        }
    }

    /// The renderer this tool is required by (each tool maps to exactly one).
    #[must_use]
    pub const fn primary_renderer(self) -> DiagramRenderer {
        match self {
            Self::Dot => DiagramRenderer::Dot,
            Self::Mmdc => DiagramRenderer::Mermaid,
            Self::WaveDrom => DiagramRenderer::WaveDrom,
            Self::Latex | Self::Dvisvgm => DiagramRenderer::TikZ,
        }
    }
}

// ---------------------------------------------------------------------------
// Tool resolution
// ---------------------------------------------------------------------------

/// A located external executable.
#[derive(Debug, Clone)]
pub struct ResolvedTool {
    pub path: PathBuf,
    /// Windows-only: the executable is a `.cmd`/`.bat` script which must be
    /// run through `cmd.exe`.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub is_cmd_script: bool,
}

#[derive(Debug, Snafu)]
pub enum ToolResolutionError {
    #[snafu(display(
        "explicit executable '{}' for tool '{}' does not exist",
        path.display(),
        tool
    ))]
    ExplicitMissing { tool: ToolName, path: PathBuf },

    #[snafu(display("executable '{}' was not found on PATH: {}", tool, hint))]
    NotFoundOnPath { tool: ToolName, hint: &'static str },
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Locate `tool`, preferring `explicit` when given, else scanning the `PATH`.
pub fn resolve_tool(
    tool: ToolName,
    explicit: Option<&Path>,
) -> Result<ResolvedTool, ToolResolutionError> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(ResolvedTool {
                path: path.to_path_buf(),
                is_cmd_script: is_cmd_script(path),
            });
        }
        return Err(ToolResolutionError::ExplicitMissing {
            tool,
            path: path.to_path_buf(),
        });
    }

    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    #[cfg(windows)]
    let found = find_in_paths(tool.as_str(), &dirs, &pathext_extensions());
    #[cfg(not(windows))]
    let found = find_in_paths(tool.as_str(), &dirs, &[]);

    found.map_or_else(
        || {
            Err(ToolResolutionError::NotFoundOnPath {
                tool,
                hint: tool.install_hint(),
            })
        },
        |path| {
            let is_cmd_script = is_cmd_script(&path);
            Ok(ResolvedTool {
                path,
                is_cmd_script,
            })
        },
    )
}

/// Scan directories for an executable named `name`, trying each extension in
/// order on Windows. Parameterized over dirs/extensions for testability (the
/// `env`-reading wrapper above stays thin and untested).
pub fn find_in_paths(name: &str, dirs: &[PathBuf], extensions: &[String]) -> Option<PathBuf> {
    for dir in dirs {
        #[cfg(windows)]
        {
            for ext in extensions {
                let candidate = dir.join(format!("{name}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = extensions;
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Windows `PATHEXT` extension list (`.COM;.EXE;...`), lowercased for a
/// case-insensitive filesystem, with a sane default when unset.
#[cfg(windows)]
fn pathext_extensions() -> Vec<String> {
    std::env::var_os("PATHEXT").map_or_else(
        || {
            vec![
                String::from(".com"),
                String::from(".exe"),
                String::from(".bat"),
                String::from(".cmd"),
            ]
        },
        |value| {
            value
                .to_string_lossy()
                .split(';')
                .filter(|ext| !ext.is_empty())
                .map(str::to_lowercase)
                .collect()
        },
    )
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).map_or(false, |metadata| {
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    })
}

// The path is only consulted on Windows; the Unix body is a constant, which
// also makes the function trivially const-able there.
#[cfg_attr(not(windows), allow(unused_variables, clippy::missing_const_for_fn))]
fn is_cmd_script(path: &Path) -> bool {
    #[cfg(windows)]
    {
        path.extension()
            .and_then(OsStr::to_str)
            .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Debug-only override of a tool's executable path via
/// `OBSIDIAN_EXPORT_DIAGRAM_BIN_<TOOL>`; used by integration tests to inject
/// mock renderers. Compiled out of release builds entirely.
fn debug_env_override(tool: ToolName) -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let key = format!("OBSIDIAN_EXPORT_DIAGRAM_BIN_{}", tool.env_key());
        std::env::var_os(key).map(PathBuf::from)
    }
    #[cfg(not(debug_assertions))]
    {
        // The parameter only feeds the debug branch above.
        let _ = tool;
        None
    }
}

// ---------------------------------------------------------------------------
// Process execution
// ---------------------------------------------------------------------------

/// Build the `Command` for a resolved tool. Command scripts (`.cmd`/`.bat`)
/// are wrapped in `cmd.exe /c` with a hand-quoted command line: `std`'s
/// automatic argument quoting does not compose with cmd.exe's parser, and
/// unquoted special characters (`&`, spaces, CJK) would break it.
pub fn build_command(tool: &ResolvedTool, args: &[OsString]) -> Command {
    #[cfg(windows)]
    if tool.is_cmd_script {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new("cmd.exe");
        command.raw_arg("/c");
        command.raw_arg(cmd_wrapper_line(&tool.path, args));
        return command;
    }

    let mut command = Command::new(&tool.path);
    command.args(args);
    command
}

/// The `cmd.exe` command line handed to `/c`: every component individually
/// double-quoted, with an extra outer pair of quotes. `/C` strips the first
/// and last quote character of its operand (see `cmd /?`), so the outer pair
/// is sacrificed to that rule, leaving each component properly quoted. Quote
/// characters cannot occur in paths (illegal in Windows filenames), so
/// quoting is always safe here.
// Windows-only by design; other platforms have no cmd.exe shims to wrap.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn cmd_wrapper_line(script: &Path, args: &[OsString]) -> OsString {
    let mut line = OsString::from("\"\"");
    line.push(script.as_os_str());
    line.push("\"");
    for arg in args {
        line.push(" \"");
        line.push(arg);
        line.push("\"");
    }
    line.push("\"");
    line
}

#[derive(Debug)]
pub struct ToolOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Snafu)]
pub enum ToolRunError {
    #[snafu(display("failed to start: {source}"))]
    Spawn { source: std::io::Error },
    #[snafu(display("failed to wait for exit: {source}"))]
    Wait { source: std::io::Error },
    #[snafu(display("timed out after {} seconds", seconds))]
    Timeout { seconds: u64 },
}
/// Run a command to completion with piped stdio and a timeout, draining the
/// pipes on reader threads so a chatty child can never deadlock on a full
/// pipe buffer. `stdin_data`, when given, is written to the child's stdin.
#[allow(clippy::arithmetic_side_effects)]
pub fn run_command(
    mut command: Command,
    stdin_data: Option<&[u8]>,
    timeout: Duration,
) -> Result<ToolOutput, ToolRunError> {
    command.stdin(if stdin_data.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().context(SpawnSnafu)?;

    // Feed stdin on a detached writer thread: the child may exit without
    // reading it all (broken pipe is fine, the error is deliberately
    // ignored). Detached like the readers — a grandchild inheriting the
    // read end could otherwise block write_all past the child's death.
    if let (Some(data), Some(mut stdin)) = (stdin_data, child.stdin.take()) {
        let data = data.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&data);
        });
    }

    // Readers deliver through a channel so the collecting side can wait with
    // a deadline: on timeout the pipe may still be held open by a grandchild
    // (see kill_process_tree), and joining a blocked reader would hang.
    let (stdout_tx, stdout_rx) = mpsc::channel();
    if let Some(mut pipe) = child.stdout.take() {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = pipe.read_to_end(&mut buffer);
            let _ = stdout_tx.send(buffer);
        });
    }
    let (stderr_tx, stderr_rx) = mpsc::channel();
    if let Some(mut pipe) = child.stderr.take() {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = pipe.read_to_end(&mut buffer);
            let _ = stderr_tx.send(buffer);
        });
    }

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().context(WaitSnafu)? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                kill_process_tree(child.id());
                let _ = child.kill();
                // Reap so the process handle and pipes are released before
                // collecting the reader threads.
                let _ = child.wait().context(WaitSnafu)?;
                let _ = collect_reader(&stdout_rx);
                let _ = collect_reader(&stderr_rx);
                return Err(ToolRunError::Timeout {
                    seconds: timeout.as_secs(),
                });
            }
            None => thread::sleep(Duration::from_millis(25)),
        }
    };

    let stdout = collect_reader(&stdout_rx);
    let stderr = collect_reader(&stderr_rx);
    Ok(ToolOutput {
        status,
        stdout,
        stderr,
    })
}

/// Wait for one reader thread's buffer, bounded by [`READER_GRACE`]. An
/// empty buffer is returned when the deadline passes (the reader is left
/// detached) or when there was no pipe to begin with.
fn collect_reader(rx: &mpsc::Receiver<Vec<u8>>) -> Vec<u8> {
    rx.recv_timeout(READER_GRACE).unwrap_or_default()
}

/// Best-effort kill of a process together with everything it spawned.
/// `child.kill()` only terminates the direct child, which for `.cmd` shims
/// is `cmd.exe` itself: the actual renderer would survive as a grandchild,
/// still holding the stdout/stderr pipe handles open. On Windows `taskkill
/// /T /F` walks the tree and takes them all down. On Unix, npm shims are
/// exec'd directly (the direct kill reaches the renderer), so the bounded
/// [`READER_GRACE`] is the only fallback kept there.
// The Unix body is empty (npm shims are exec'd directly), which makes the
// function trivially const-able there.
#[cfg_attr(not(windows), allow(clippy::missing_const_for_fn))]
fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        // Spawn-and-forget: waiting on taskkill itself could stall on a
        // target stuck in uninterruptible kernel mode. It finishes in
        // milliseconds in practice, and the direct kill below reaps what
        // it missed either way.
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID"])
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
    }
}

// ---------------------------------------------------------------------------
// Render pipeline
// ---------------------------------------------------------------------------

/// All tools required by the enabled renderers, resolved once during the
/// export prescan and shared across worker threads.
#[derive(Debug, Clone)]
pub struct DiagramToolset {
    tools: BTreeMap<ToolName, ResolvedTool>,
}

impl DiagramToolset {
    /// Resolve every tool needed by `renderers`. `bins` carries explicit
    /// per-tool executable paths (highest priority below the debug-only test
    /// hook); everything else falls back to a `PATH` scan.
    pub fn resolve(
        renderers: &[DiagramRenderer],
        bins: &BTreeMap<ToolName, PathBuf>,
    ) -> Result<Self, ToolResolutionError> {
        let mut needed: BTreeSet<ToolName> = BTreeSet::new();
        for renderer in renderers {
            needed.extend(renderer.tool_names().iter().copied());
        }

        let mut tools = BTreeMap::new();
        for tool in needed {
            let explicit = debug_env_override(tool).or_else(|| bins.get(&tool).cloned());
            let resolved = resolve_tool(tool, explicit.as_deref())?;
            tools.insert(tool, resolved);
        }
        Ok(Self { tools })
    }

    fn get(&self, tool: ToolName) -> Result<&ResolvedTool, DiagramRenderError> {
        self.tools
            .get(&tool)
            .ok_or(DiagramRenderError::ToolMissing { tool })
    }
}

/// Shared, thread-safe rendering state for one export run.
pub struct DiagramState {
    toolset: DiagramToolset,
    format: DiagramFormat,
    /// The renderers the user enabled; code blocks mapping to any other
    /// renderer (or none) pass through untouched.
    enabled: Vec<DiagramRenderer>,
    /// Total number of renderable blocks found by the prescan, for progress
    /// reporting (`index/total`).
    total: usize,
    done: AtomicUsize,
    next_tmp_id: AtomicU32,
}

impl DiagramState {
    pub const fn new(
        toolset: DiagramToolset,
        format: DiagramFormat,
        enabled: Vec<DiagramRenderer>,
        total: usize,
    ) -> Self {
        Self {
            toolset,
            format,
            enabled,
            total,
            done: AtomicUsize::new(0),
            next_tmp_id: AtomicU32::new(0),
        }
    }
}

/// The outcome of rendering one diagram block.
pub struct RenderedAsset {
    /// Absolute path of the written (or cached) asset file.
    pub path: PathBuf,
    /// Format actually produced (may differ from the requested one after
    /// fallback).
    pub format: DiagramFormat,
    /// True when the asset already existed (content-addressed name hit) and
    /// rendering was skipped. Carried for callers and tests; the event
    /// stream deliberately does not distinguish cache hits.
    #[allow(dead_code)]
    pub from_cache: bool,
}

#[derive(Debug, Snafu)]
pub enum DiagramRenderError {
    #[snafu(display("tool '{}' unavailable", tool))]
    ToolMissing { tool: ToolName },

    #[snafu(display("failed to run '{}': {source}", tool.display()))]
    Run { tool: PathBuf, source: ToolRunError },

    #[snafu(display(
        "'{}' exited with {status}: {detail}",
        tool.display()
    ))]
    ExitStatus {
        tool: PathBuf,
        status: String,
        detail: String,
    },

    #[snafu(display(
        "'{}' produced no {} output",
        tool.display(),
        expected
    ))]
    OutputMissing {
        tool: PathBuf,
        expected: &'static str,
    },

    #[snafu(display("{context}: {source}"))]
    Io {
        context: String,
        source: std::io::Error,
    },
}

/// Render one diagram `source` into `assets_dir` next to a note.
///
/// The target filename is content-addressed (`<note>-<16 hex>.<ext>` over the
/// renderer, language, source text and effective format), so an unchanged
/// block resolves to the same file across runs and re-exports skip the
/// external tool entirely. Rendering goes to a dot-prefixed temporary file in
/// `assets_dir` first and is renamed into place only on success.
#[allow(clippy::too_many_lines)]
pub fn render_to_asset(
    renderer: DiagramRenderer,
    language: &str,
    source: &str,
    requested: DiagramFormat,
    state: &DiagramState,
    assets_dir: &Path,
    note_stem: &str,
) -> Result<RenderedAsset, DiagramRenderError> {
    let format = renderer.effective_format(requested);
    let target = assets_dir.join(asset_filename(
        note_stem, renderer, language, source, format,
    ));

    if target.is_file() {
        return Ok(RenderedAsset {
            path: target,
            format,
            from_cache: true,
        });
    }

    fs::create_dir_all(assets_dir).context(IoSnafu {
        context: format!(
            "failed to create assets directory '{}'",
            assets_dir.display()
        ),
    })?;

    let tmp_out = assets_dir.join(format!(
        ".render-{}-{}.{}",
        std::process::id(),
        state.next_tmp_id.fetch_add(1, Ordering::Relaxed),
        format.as_str()
    ));

    let result = render_with_tool(renderer, source, format, state, &tmp_out);
    match result {
        Ok(()) => {
            fs::rename(&tmp_out, &target)
                .or_else(|_| {
                    // Rename can fail across filesystems; fall back to a copy
                    // plus removal, which loses atomicity but still lands the
                    // complete file. Unreachable today (the temporary file
                    // lives in the same directory as the target) and kept as
                    // defense in case the tmp placement ever changes. Once
                    // the copy has landed, failing to remove the leftover
                    // temporary file must not fail the render: the asset
                    // itself is complete.
                    fs::copy(&tmp_out, &target)
                        .and_then(|_| fs::remove_file(&tmp_out).or(Ok(())))
                        .inspect_err(|_| {
                            // A half-written target would be cache-hit as a
                            // corrupt asset on the next run; remove it so
                            // this failure stays a failure.
                            let _ = fs::remove_file(&target);
                        })
                })
                .context(IoSnafu {
                    context: format!("failed to move asset into place at '{}'", target.display()),
                })?;
            Ok(RenderedAsset {
                path: target,
                format,
                from_cache: false,
            })
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp_out);
            Err(error)
        }
    }
}

fn render_with_tool(
    renderer: DiagramRenderer,
    source: &str,
    format: DiagramFormat,
    state: &DiagramState,
    tmp_out: &Path,
) -> Result<(), DiagramRenderError> {
    let workdir = tempfile::tempdir().context(IoSnafu {
        context: String::from("failed to create rendering work directory"),
    })?;
    let workdir = workdir.path();

    match renderer {
        DiagramRenderer::Dot => render_dot(state, source, format, workdir, tmp_out),
        DiagramRenderer::Mermaid => {
            let input = workdir.join("diagram.mmd");
            fs::write(&input, source).context(IoSnafu {
                context: String::from("failed to write mermaid input"),
            })?;
            render_mermaid(state, &input, tmp_out)
        }
        DiagramRenderer::WaveDrom => {
            let input = workdir.join("diagram.json5");
            fs::write(&input, source).context(IoSnafu {
                context: String::from("failed to write wavedrom input"),
            })?;
            render_wavedrom(state, &input, tmp_out)
        }
        DiagramRenderer::TikZ => render_tikz(state, source, workdir, tmp_out),
    }
}

fn render_dot(
    state: &DiagramState,
    source: &str,
    format: DiagramFormat,
    workdir: &Path,
    tmp_out: &Path,
) -> Result<(), DiagramRenderError> {
    let dot = state.toolset.get(ToolName::Dot)?;
    // Source goes through a file rather than stdin: graphviz reads either,
    // but a piped stdin is unreliable across the cmd.exe wrapper used for
    // .cmd shims (cmd.exe pre-reads a chunk of the pipe).
    let input = workdir.join("diagram.dot");
    fs::write(&input, source).context(IoSnafu {
        context: String::from("failed to write dot input"),
    })?;
    let output_format = match format {
        DiagramFormat::Svg => "-Tsvg",
        DiagramFormat::Png => "-Tpng",
    };
    let args = vec![
        OsString::from(output_format),
        input.as_os_str().to_owned(),
        OsString::from("-o"),
        tmp_out.as_os_str().to_owned(),
    ];
    run_renderer_tool(dot, &args)
}

fn render_mermaid(
    state: &DiagramState,
    input: &Path,
    tmp_out: &Path,
) -> Result<(), DiagramRenderError> {
    let mmdc = state.toolset.get(ToolName::Mmdc)?;
    let args = vec![
        OsString::from("-i"),
        input.as_os_str().to_owned(),
        OsString::from("-o"),
        tmp_out.as_os_str().to_owned(),
    ];
    run_renderer_tool(mmdc, &args)
}

fn render_wavedrom(
    state: &DiagramState,
    input: &Path,
    tmp_out: &Path,
) -> Result<(), DiagramRenderError> {
    let wavedrom = state.toolset.get(ToolName::WaveDrom)?;
    let args = vec![OsString::from("--input"), input.as_os_str().to_owned()];
    // wavedrom writes the SVG to stdout instead of an output flag.
    let output =
        run_command(build_command(wavedrom, &args), None, TOOL_TIMEOUT).context(RunSnafu {
            tool: wavedrom.path.clone(),
        })?;
    if !output.status.success() {
        return Err(DiagramRenderError::ExitStatus {
            tool: wavedrom.path.clone(),
            status: output.status.to_string(),
            detail: tail_utf8(&output.stderr, 800),
        });
    }
    if output.stdout.is_empty() {
        return Err(DiagramRenderError::OutputMissing {
            tool: wavedrom.path.clone(),
            expected: "SVG on stdout",
        });
    }
    fs::write(tmp_out, &output.stdout).context(IoSnafu {
        context: format!("failed to write wavedrom output '{}'", tmp_out.display()),
    })
}

fn render_tikz(
    state: &DiagramState,
    source: &str,
    workdir: &Path,
    tmp_out: &Path,
) -> Result<(), DiagramRenderError> {
    let tex = workdir.join("diagram.tex");
    fs::write(&tex, tikz_wrapper(source)).context(IoSnafu {
        context: String::from("failed to write tikz input"),
    })?;

    // Step 1: latex (DVI route; dvisvgm renders DVI with better font
    // handling than PDF). Errors surface in diagram.log more reliably than
    // on stderr, so the log tail backs up the stderr tail.
    let latex = state.toolset.get(ToolName::Latex)?;
    let latex_args = vec![
        OsString::from("-interaction=nonstopmode"),
        OsString::from("-halt-on-error"),
        OsString::from("-output-directory"),
        workdir.as_os_str().to_owned(),
        tex.as_os_str().to_owned(),
    ];
    let latex_output =
        run_command(build_command(latex, &latex_args), None, TOOL_TIMEOUT).context(RunSnafu {
            tool: latex.path.clone(),
        })?;
    if !latex_output.status.success() {
        let mut detail = tail_utf8(&latex_output.stderr, 800);
        if detail.is_empty() {
            if let Ok(log) = fs::read(workdir.join("diagram.log")) {
                detail = tail_utf8(&log, 1600);
            }
        }
        return Err(DiagramRenderError::ExitStatus {
            tool: latex.path.clone(),
            status: latex_output.status.to_string(),
            detail,
        });
    }

    let dvi = workdir.join("diagram.dvi");
    if !dvi.is_file() {
        return Err(DiagramRenderError::OutputMissing {
            tool: latex.path.clone(),
            expected: "diagram.dvi",
        });
    }

    // Step 2: dvisvgm with fonts converted to paths (--no-fonts): SVG font
    // elements are poorly supported outside a few renderers.
    let dvisvgm = state.toolset.get(ToolName::Dvisvgm)?;
    let dvisvgm_args = vec![
        OsString::from("--no-fonts"),
        OsString::from("--exact"),
        OsString::from("-o"),
        tmp_out.as_os_str().to_owned(),
        dvi.as_os_str().to_owned(),
    ];
    run_renderer_tool(dvisvgm, &dvisvgm_args)
}

fn run_renderer_tool(tool: &ResolvedTool, args: &[OsString]) -> Result<(), DiagramRenderError> {
    let output = run_command(build_command(tool, args), None, TOOL_TIMEOUT).context(RunSnafu {
        tool: tool.path.clone(),
    })?;
    if !output.status.success() {
        return Err(DiagramRenderError::ExitStatus {
            tool: tool.path.clone(),
            status: output.status.to_string(),
            detail: tail_utf8(&output.stderr, 800),
        });
    }
    Ok(())
}

/// Wrap user tikz code in a standalone document so the page is cropped to
/// the drawing. Block content is the *inside* of a `tikzpicture` environment
/// (Obsidian plugin convention), so the environment is added here — unless
/// the source already carries one, in which case it is embedded verbatim.
#[must_use]
pub fn tikz_wrapper(source: &str) -> String {
    let body = if source.contains("\\begin{tikzpicture}") {
        String::from(source)
    } else {
        format!("\\begin{{tikzpicture}}\n{source}\n\\end{{tikzpicture}}")
    };
    format!(
        "\\documentclass{{standalone}}\n\\usepackage{{tikz}}\n\\begin{{document}}\n{body}\n\\end{{document}}\n"
    )
}

// ---------------------------------------------------------------------------
// Event stream rewriting
// ---------------------------------------------------------------------------

/// Replace diagram code blocks in a note's event stream with image
/// references, rendering through the tools resolved in `state`.
///
/// Failures are non-fatal by design: a block whose code the external tool
/// rejects stays a code block and produces a warning, so the export always
/// completes. `on_event` receives `ExportEvent::DiagramRender` progress.
#[allow(clippy::arithmetic_side_effects)]
pub fn process_diagram_events(
    state: &DiagramState,
    context: &Context,
    events: &mut MarkdownEvents<'_>,
    on_event: &dyn Fn(&ExportEvent),
    on_warning: &dyn Fn(String),
) {
    let Some(assets_dir) = context
        .destination
        .parent()
        .map(|parent| parent.join("assets"))
    else {
        return;
    };
    let note_stem = context.destination.file_stem().map_or_else(
        || String::from("note"),
        |stem| stem.to_string_lossy().into_owned(),
    );

    let mut index = 0;
    while let Some(event) = events.get(index) {
        let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = event else {
            index += 1;
            continue;
        };

        let Some(renderer) = DiagramRenderer::from_language(info) else {
            index += 1;
            continue;
        };
        if !state.enabled.contains(&renderer) {
            index += 1;
            continue;
        }

        // Collect the block's text events up to the closing End(CodeBlock).
        let mut end = index + 1;
        let mut source = String::new();
        let mut closed = false;
        while let Some(inner) = events.get(end) {
            match inner {
                Event::Text(text) => {
                    source.push_str(text);
                    end += 1;
                }
                Event::End(TagEnd::CodeBlock) => {
                    closed = true;
                    break;
                }
                _ => {
                    end += 1;
                }
            }
        }
        if !closed {
            // Malformed stream (no closing fence): leave it untouched.
            index += 1;
            continue;
        }

        let language = String::from(info.split_whitespace().next().unwrap_or_default());
        let progress = state.done.fetch_add(1, Ordering::Relaxed) + 1;
        on_event(&ExportEvent::DiagramRender {
            language: language.clone(),
            index: progress,
            total: state.total,
        });

        match render_to_asset(
            renderer,
            &language,
            &source,
            state.format,
            state,
            &assets_dir,
            &note_stem,
        ) {
            Ok(asset) => {
                if asset.format != state.format {
                    on_warning(format!(
                        "renderer '{}' cannot produce {} output; fell back to {} for a diagram in '{}'",
                        renderer.name(),
                        state.format.as_str(),
                        asset.format.as_str(),
                        context.destination.display(),
                    ));
                }
                let url = image_url(&asset.path, &context.destination);
                let alt = format!("diagram ({language})");
                // The trailing SoftBreak separates consecutive diagrams:
                // the replaced code blocks were block-level and carried no
                // paragraph events between them, so back-to-back images
                // would otherwise render on one glued-together line.
                let replacement = vec![
                    Event::Start(Tag::Image {
                        link_type: pulldown_cmark::LinkType::Inline,
                        dest_url: CowStr::from(url),
                        title: CowStr::from(""),
                        id: CowStr::from(""),
                    }),
                    Event::Text(CowStr::from(alt)),
                    Event::End(TagEnd::Image),
                    Event::SoftBreak,
                ];
                let replacement_len = replacement.len();
                events.splice(index..=end, replacement);
                index += replacement_len;
            }
            Err(error) => {
                on_warning(format!(
                    "failed to render {} diagram in '{}': {error}",
                    language,
                    context.destination.display(),
                ));
                index = end + 1;
            }
        }
    }
}

/// Markdown image URL for an asset, relative to the note referencing it.
fn image_url(asset: &Path, note_destination: &Path) -> String {
    let relative = diff_paths(
        asset,
        note_destination.parent().unwrap_or_else(|| Path::new("")),
    )
    .unwrap_or_else(|| asset.to_path_buf());
    let relative = relative.to_string_lossy().replace('\\', "/");
    encode_link_destination(&relative)
}

// ---------------------------------------------------------------------------
// Prescan
// ---------------------------------------------------------------------------

/// Outcome of scanning a note's source for diagram code blocks.
#[derive(Default)]
pub struct PrescanHit {
    /// Renderable blocks counting towards progress (enabled languages only).
    pub renderable_blocks: usize,
    /// Languages of those blocks, to derive the required tool set.
    pub renderers: BTreeSet<DiagramRenderer>,
}

/// Scan note source text for fenced code blocks that map to one of the
/// enabled renderers. Uses a plain pulldown-cmark pass; Obsidian-specific
/// syntax is irrelevant to fence detection.
#[allow(clippy::arithmetic_side_effects)]
pub fn prescan_note(text: &str, enabled: &[DiagramRenderer]) -> PrescanHit {
    let mut hit = PrescanHit::default();
    for event in Parser::new(text) {
        if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = event {
            if let Some(renderer) = DiagramRenderer::from_language(&info) {
                if enabled.contains(&renderer) {
                    hit.renderable_blocks += 1;
                    hit.renderers.insert(renderer);
                }
            }
        }
    }
    hit
}

// ---------------------------------------------------------------------------
// Content-addressed naming
// ---------------------------------------------------------------------------

/// FNV-1a 64-bit: deterministic across compiler and crate versions (unlike
/// `std`'s `DefaultHasher`), and plenty unique for content-addressed filenames.
#[derive(Default)]
struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn update(&mut self, data: &[u8]) {
        let mut hash = if self.0 == 0 {
            Self::OFFSET_BASIS
        } else {
            self.0
        };
        for &byte in data {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(Self::PRIME);
        }
        self.0 = hash;
    }

    const fn finish(self) -> u64 {
        if self.0 == 0 {
            Self::OFFSET_BASIS
        } else {
            self.0
        }
    }
}

/// Asset filename: `<note stem>-<16 hex digest>.<ext>`, digest over renderer,
/// language, source text and effective format. The stem is truncated to keep
/// the total length bounded on Windows.
#[must_use]
pub fn asset_filename(
    note_stem: &str,
    renderer: DiagramRenderer,
    language: &str,
    source: &str,
    format: DiagramFormat,
) -> String {
    let mut hasher = Fnv1a64::default();
    hasher.update(renderer.name().as_bytes());
    hasher.update(&[0]);
    hasher.update(language.as_bytes());
    hasher.update(&[0]);
    hasher.update(source.as_bytes());
    hasher.update(&[0]);
    hasher.update(format.as_str().as_bytes());

    let stem: String = note_stem.chars().take(ASSET_STEM_MAX_CHARS).collect();
    format!("{stem}-{:016x}.{}", hasher.finish(), format.as_str())
}

/// Lossy UTF-8 rendering of the last `max` bytes of `output`.
fn tail_utf8(output: &[u8], max: usize) -> String {
    let start = output.len().saturating_sub(max);
    String::from_utf8_lossy(output.get(start..).unwrap_or(&[])).into_owned()
}

#[cfg(test)]
#[allow(clippy::case_sensitive_file_extension_comparisons)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(&[], 0xcbf2_9ce4_8422_2325)]
    #[case(b"a", 0xaf63_dc4c_8601_ec8c)]
    #[case(b"foobar", 0x8594_4171_f739_67e8)]
    fn fnv1a64_known_vectors(#[case] data: &[u8], #[case] expected: u64) {
        let mut hasher = Fnv1a64::default();
        hasher.update(data);
        assert_eq!(hasher.finish(), expected);
    }

    #[rstest]
    #[case("dot", Some(DiagramRenderer::Dot))]
    #[case("graphviz", Some(DiagramRenderer::Dot))]
    #[case("mermaid", Some(DiagramRenderer::Mermaid))]
    #[case("mmd", Some(DiagramRenderer::Mermaid))]
    #[case("wavedrom", Some(DiagramRenderer::WaveDrom))]
    #[case("tikz", Some(DiagramRenderer::TikZ))]
    #[case("DOT", Some(DiagramRenderer::Dot))]
    #[case("TikZ", Some(DiagramRenderer::TikZ))]
    #[case("mermaid init", Some(DiagramRenderer::Mermaid))]
    #[case("rust", None)]
    #[case("", None)]
    fn from_language(#[case] language: &str, #[case] expected: Option<DiagramRenderer>) {
        assert_eq!(DiagramRenderer::from_language(language), expected);
    }

    #[rstest]
    #[case(DiagramRenderer::Dot, DiagramFormat::Svg, true)]
    #[case(DiagramRenderer::Dot, DiagramFormat::Png, true)]
    #[case(DiagramRenderer::Mermaid, DiagramFormat::Png, true)]
    #[case(DiagramRenderer::WaveDrom, DiagramFormat::Svg, true)]
    #[case(DiagramRenderer::WaveDrom, DiagramFormat::Png, false)]
    #[case(DiagramRenderer::TikZ, DiagramFormat::Png, false)]
    fn format_support(
        #[case] renderer: DiagramRenderer,
        #[case] format: DiagramFormat,
        #[case] supported: bool,
    ) {
        assert_eq!(renderer.supports(format), supported);
        let effective = renderer.effective_format(format);
        assert_eq!(effective == format, supported);
    }

    #[rstest]
    #[case(DiagramRenderer::Dot, &[ToolName::Dot])]
    #[case(DiagramRenderer::Mermaid, &[ToolName::Mmdc])]
    #[case(DiagramRenderer::WaveDrom, &[ToolName::WaveDrom])]
    #[case(DiagramRenderer::TikZ, &[ToolName::Latex, ToolName::Dvisvgm])]
    fn tool_requirements(#[case] renderer: DiagramRenderer, #[case] tools: &[ToolName]) {
        assert_eq!(renderer.tool_names(), tools);
    }

    #[test]
    fn tool_name_roundtrip() {
        for tool in [
            ToolName::Dot,
            ToolName::Mmdc,
            ToolName::WaveDrom,
            ToolName::Latex,
            ToolName::Dvisvgm,
        ] {
            assert_eq!(ToolName::from_name(tool.as_str()), Some(tool));
        }
        assert_eq!(ToolName::from_name("inkscape"), None);
    }

    #[test]
    fn asset_filename_shape_and_stability() {
        let a = asset_filename(
            "note",
            DiagramRenderer::Mermaid,
            "mermaid",
            "graph TD; A-->B",
            DiagramFormat::Svg,
        );
        assert_eq!(a, "note-1f555c3f4c133669.svg");
        // Same inputs, same name; any input change, different name.
        let a2 = asset_filename(
            "note",
            DiagramRenderer::Mermaid,
            "mermaid",
            "graph TD; A-->B",
            DiagramFormat::Svg,
        );
        assert_eq!(a, a2);
        let b = asset_filename(
            "note",
            DiagramRenderer::Mermaid,
            "mermaid",
            "graph TD; A-->C",
            DiagramFormat::Svg,
        );
        assert_ne!(a, b);
        let c = asset_filename(
            "other",
            DiagramRenderer::Mermaid,
            "mermaid",
            "graph TD; A-->B",
            DiagramFormat::Svg,
        );
        assert_ne!(a, c);
        let d = asset_filename(
            "note",
            DiagramRenderer::Mermaid,
            "mermaid",
            "graph TD; A-->B",
            DiagramFormat::Png,
        );
        assert_ne!(a, d);
        assert!(d.ends_with(".png"));
    }

    #[test]
    fn asset_filename_truncates_long_stems() {
        let long_stem = "x".repeat(300);
        let name = asset_filename(
            &long_stem,
            DiagramRenderer::Dot,
            "dot",
            "digraph {}",
            DiagramFormat::Svg,
        );
        // stem + '-' + 16 hex + '.svg'
        assert_eq!(name.len(), ASSET_STEM_MAX_CHARS + 1 + 16 + 4);
        assert!(name.starts_with(long_stem.get(..ASSET_STEM_MAX_CHARS).unwrap_or_default()));
    }

    #[test]
    fn prescan_counts_only_enabled_languages() {
        let text = "\
# Title

```dot
digraph { a -> b }
```

```mermaid
graph TD; A-->B
```

```rust
fn main() {}
```
";
        let hit = prescan_note(text, &[DiagramRenderer::Dot, DiagramRenderer::Mermaid]);
        assert_eq!(hit.renderable_blocks, 2);
        assert_eq!(
            hit.renderers,
            BTreeSet::from([DiagramRenderer::Dot, DiagramRenderer::Mermaid])
        );

        let dot_only = prescan_note(text, &[DiagramRenderer::Dot]);
        assert_eq!(dot_only.renderable_blocks, 1);
        assert_eq!(dot_only.renderers, BTreeSet::from([DiagramRenderer::Dot]));

        let none = prescan_note(text, &[]);
        assert_eq!(none.renderable_blocks, 0);
        assert!(none.renderers.is_empty());
    }

    #[test]
    fn prescan_ignores_indented_code_blocks() {
        // Indented (non-fenced) code blocks carry no info string and are not
        // Obsidian render targets.
        let text = "text\n\n    dot code here\n";
        let hit = prescan_note(text, &[DiagramRenderer::Dot]);
        assert_eq!(hit.renderable_blocks, 0);
    }

    #[test]
    fn tikz_wrapper_is_standalone() {
        let wrapper = tikz_wrapper("\\draw (0,0) -- (1,1);");
        assert!(wrapper.contains("\\documentclass{standalone}"));
        assert!(wrapper.contains("\\usepackage{tikz}"));
        assert!(wrapper.contains("\\begin{document}"));
        // Bare drawing commands get wrapped in a tikzpicture environment.
        assert!(wrapper.contains("\\begin{tikzpicture}"));
        assert!(wrapper.contains("\\draw (0,0) -- (1,1);"));
        assert!(wrapper.contains("\\end{tikzpicture}"));
        assert!(wrapper.contains("\\end{document}"));

        // Source carrying its own environment is embedded verbatim (no
        // double wrapping).
        let prewrapped = tikz_wrapper("\\begin{tikzpicture}\\draw;\\end{tikzpicture}");
        assert_eq!(
            prewrapped.matches("\\begin{tikzpicture}").count(),
            1,
            "{prewrapped}"
        );
    }

    #[test]
    fn cmd_wrapper_line_quotes_every_component() {
        let script = Path::new("C:\\Tools\\my mmdc.cmd");
        let args = vec![
            OsString::from("-i"),
            OsString::from("D:\\vault 图\\a & b.mmd"),
        ];
        let line = cmd_wrapper_line(script, &args)
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            line,
            "\"\"C:\\Tools\\my mmdc.cmd\" \"-i\" \"D:\\vault 图\\a & b.mmd\"\""
        );
    }

    #[test]
    fn image_url_encodes_like_link_destinations() {
        let asset = Path::new("out/sub/assets/note-abc.svg");
        let note = Path::new("out/sub/note.md");
        assert_eq!(image_url(asset, note), "assets/note-abc.svg");

        // A stem with a space must be percent-encoded, matching what
        // wikilink destinations get.
        let spaced_asset = Path::new("out/my dir/assets/note (1)-abc.svg");
        let spaced_note = Path::new("out/my dir/note (1).md");
        assert_eq!(
            image_url(spaced_asset, spaced_note),
            "assets/note%20%281%29-abc.svg"
        );
    }

    #[test]
    fn find_in_paths_scans_directories_in_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        fs::create_dir_all(&dir_a).expect("mkdir a");
        fs::create_dir_all(&dir_b).expect("mkdir b");

        #[cfg(windows)]
        {
            fs::write(dir_a.join("dot.exe"), "bin").expect("write dot.exe");
            fs::write(dir_b.join("dot.cmd"), "bin").expect("write dot.cmd");

            let found = find_in_paths(
                "dot",
                &[dir_b.clone(), dir_a.clone()],
                &[String::from(".exe"), String::from(".cmd")],
            );
            assert_eq!(found, Some(dir_b.join("dot.cmd")));

            let found_preferring_a = find_in_paths(
                "dot",
                &[dir_a.clone(), dir_b],
                &[String::from(".exe"), String::from(".cmd")],
            );
            assert_eq!(found_preferring_a, Some(dir_a.join("dot.exe")));

            assert_eq!(
                find_in_paths("missing", &[dir_a], &[String::from(".exe")]),
                None
            );
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;

            let executable = dir_a.join("dot");
            fs::write(&executable, "#!/bin/sh\n").expect("write dot");
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).expect("chmod");
            let plain = dir_b.join("dot");
            fs::write(plain, "not executable").expect("write plain");

            assert_eq!(
                find_in_paths("dot", &[dir_a.clone(), dir_b.clone()], &[]),
                Some(executable)
            );
            // Not executable: skipped, not matched.
            assert_eq!(find_in_paths("dot", &[dir_b], &[]), None);
            // Missing name: not found.
            assert_eq!(find_in_paths("missing", &[dir_a], &[]), None);
        }
    }

    #[test]
    fn resolve_tool_rejects_missing_explicit_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("does-not-exist.exe");
        match resolve_tool(ToolName::Dot, Some(&missing)) {
            Err(ToolResolutionError::ExplicitMissing { tool, path }) => {
                assert_eq!(tool, ToolName::Dot);
                assert_eq!(path, missing);
            }
            other => panic!("expected ExplicitMissing, got {:?}", other),
        }
    }

    #[test]
    fn resolve_tool_accepts_existing_explicit_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        #[cfg(windows)]
        let tool_file = tmp.path().join("custom-dot.exe");
        #[cfg(not(windows))]
        let tool_file = tmp.path().join("custom-dot");
        fs::write(&tool_file, "bin").expect("write tool");

        let resolved = resolve_tool(ToolName::Dot, Some(&tool_file)).expect("resolve");
        assert_eq!(resolved.path, tool_file);
    }

    #[cfg(windows)]
    #[test]
    fn is_cmd_script_detection() {
        assert!(is_cmd_script(Path::new("C:\\npm\\mmdc.cmd")));
        assert!(is_cmd_script(Path::new("C:\\npm\\mmdc.CMD")));
        assert!(is_cmd_script(Path::new("C:\\npm\\tool.bat")));
        assert!(!is_cmd_script(Path::new("C:\\scoop\\dot.exe")));
        assert!(!is_cmd_script(Path::new("C:\\npm\\mmdc")));
    }

    #[test]
    fn tail_utf8_takes_last_bytes() {
        assert_eq!(tail_utf8(b"abcdef", 3), "def");
        assert_eq!(tail_utf8(b"abc", 100), "abc");
        assert_eq!(tail_utf8(b"", 10), "");
    }

    /// A wedged tool must be reported as a timeout well before its own
    /// runtime ends — the kill/collect path (tree kill, bounded reader
    /// grace) has to return instead of hanging. On Windows the mock is a
    /// `.cmd` script, exercising the cmd.exe wrapper on the timeout path.
    ///
    /// The elapsed bound is the timeout plus at most two full reader grace
    /// periods (a CI machine can be too slow for the tree kill to land
    /// inside the grace, in which case both collectors burn their full 5s
    /// by design) with headroom for process startup; it stays far below
    /// the child's own 30s runtime, which is what a regressed
    /// implementation that waits out the child would hit.
    #[test]
    fn run_command_times_out_without_blocking() {
        let tmp = tempfile::tempdir().expect("tempdir");

        #[cfg(windows)]
        let (script, is_cmd) = {
            let script = tmp.path().join("wedge.cmd");
            // `timeout` needs an interactive console; ping is the classic
            // sleep substitute (~30s here, both streams redirected away
            // from the pipes).
            let body = "@ping -n 31 127.0.0.1 >nul 2>&1\r\n";
            fs::write(&script, body).expect("write wedge script");
            (script, true)
        };
        #[cfg(not(windows))]
        let (script, is_cmd) = {
            use std::os::unix::fs::PermissionsExt;

            let script = tmp.path().join("wedge");
            fs::write(&script, "#!/bin/sh\nexec sleep 30\n").expect("write wedge script");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");
            (script, false)
        };

        let tool = ResolvedTool {
            path: script,
            is_cmd_script: is_cmd,
        };
        let command = build_command(&tool, &[]);
        let started = Instant::now();
        let result = run_command(command, None, Duration::from_millis(150));
        assert!(
            matches!(result, Err(ToolRunError::Timeout { seconds: 0 })),
            "expected a timeout error, got {:?}",
            result
        );
        assert!(
            started.elapsed() < Duration::from_secs(13),
            "run_command must return within timeout + reader grace, not wait out the child (took {:?})",
            started.elapsed()
        );
    }

    /// A short-lived successful invocation still yields its stdout through
    /// the channel-based readers.
    #[test]
    fn run_command_collects_stdout() {
        let tmp = tempfile::tempdir().expect("tempdir");

        #[cfg(windows)]
        let (script, is_cmd) = {
            let script = tmp.path().join("hello.cmd");
            fs::write(&script, "@echo hello\r\n").expect("write script");
            (script, true)
        };
        #[cfg(not(windows))]
        let (script, is_cmd) = {
            use std::os::unix::fs::PermissionsExt;

            let script = tmp.path().join("hello");
            fs::write(&script, "#!/bin/sh\necho hello\n").expect("write script");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod");
            (script, false)
        };

        let tool = ResolvedTool {
            path: script,
            is_cmd_script: is_cmd,
        };
        let output = run_command(build_command(&tool, &[]), None, Duration::from_secs(10))
            .expect("run should succeed");
        assert!(output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stdout).trim() == "hello",
            "stdout: {:?}",
            output.stdout
        );
    }
}
