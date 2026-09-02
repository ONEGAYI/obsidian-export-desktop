// `serde_yaml` is provided by the `yaml_serde` crate via a Cargo package
// rename: upstream serde_yaml 0.9.34 is archived and unmaintained, and
// yaml_serde (maintained by the YAML org) is a fork of 0.9.34 with identical
// parsing/emitting behavior. The rename keeps the public
// `obsidian_export::serde_yaml` path source-compatible for downstream users.
pub use pulldown_cmark;
pub use serde_yaml;

mod comments;
mod context;
mod diagrams;
mod frontmatter;
mod linkcheck;
pub mod postprocessors;
mod references;
mod update;
mod walker;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{fmt, str};

pub use comments::CommentsMode;
pub use context::Context;
pub use diagrams::{DiagramFormat, DiagramRenderer, ToolName};
use diagrams::{DiagramState, DiagramToolset, ToolResolutionError};
use filetime::set_file_mtime;
use frontmatter::{frontmatter_from_str, frontmatter_to_str};
pub use frontmatter::{Frontmatter, FrontmatterStrategy};
pub use linkcheck::{CheckSummary, LinkCheckReport, LinkCheckStatus, LinkKind};
use pathdiff::diff_paths;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark_with_options;
use rayon::prelude::*;
use references::{ObsidianNoteReference, RefParser, RefParserState, RefType};
use snafu::{ResultExt, Snafu};
use unicode_normalization::UnicodeNormalization;
pub use update::{
    check_update,
    current_target_triple,
    validate_asset_name,
    write_atomic_bytes,
    AssetTarget,
    DownloadProgress,
    DownloadProgressReporter,
    ReleaseAsset,
    UpdateClient,
    UpdateError,
    UpdateStatus,
    UreqUpdateClient,
};
pub use walker::{vault_contents, WalkOptions};

/// A series of markdown [Event]s that are generated while traversing an Obsidian markdown note.
pub type MarkdownEvents<'a> = Vec<Event<'a>>;

/// A post-processing function that is to be called after an Obsidian note has been fully parsed and
/// converted to regular markdown syntax.
///
/// Postprocessors are called in the order they've been added through
/// [`Exporter::add_postprocessor`] just before notes are written out to their final destination.
/// They may be used to achieve the following:
///
/// 1. Modify a note's [Context], for example to change the destination filename or update its
///    [Frontmatter] (see [`Context::frontmatter`]).
/// 2. Change a note's contents by altering [`MarkdownEvents`].
/// 3. Prevent later postprocessors from running ([`PostprocessorResult::StopHere`]) or cause a note
///    to be skipped entirely ([`PostprocessorResult::StopAndSkipNote`]).
///
/// # Postprocessors and embeds
///
/// Postprocessors normally run at the end of the export phase, once notes have been fully parsed.
/// This means that any embedded notes have been resolved and merged into the final note already.
///
/// In some cases it may be desirable to change the contents of these embedded notes *before* they
/// are inserted into the final document. This is possible through the use of
/// [`Exporter::add_embed_postprocessor`].
/// These "embed postprocessors" run much the same way as regular postprocessors, but they're run on
/// the note that is about to be embedded in another note. In addition:
///
/// - Changes to context carry over to later embed postprocessors, but are then discarded. This
///   means that changes to frontmatter do not propagate to the root note for example.
/// - [`PostprocessorResult::StopAndSkipNote`] prevents the embedded note from being included (it's
///   replaced with a blank document) but doesn't affect the root note.
///
/// It's possible to pass the same functions to [`Exporter::add_postprocessor`] and
/// [`Exporter::add_embed_postprocessor`]. The [`Context::note_depth`] method may be used to
/// determine whether a note is a root note or an embedded note in this situation.
///
/// # Examples
///
/// ## Update frontmatter
///
/// This example shows how to make changes a note's frontmatter. In this case, the postprocessor is
/// defined inline as a closure.
///
/// ```
/// use obsidian_export::serde_yaml::Value;
/// use obsidian_export::{Exporter, PostprocessorResult};
/// # use std::path::PathBuf;
/// # use tempfile::TempDir;
///
/// # let tmp_dir = TempDir::new().expect("failed to make tempdir");
/// # let source = PathBuf::from("tests/testdata/input/postprocessors");
/// # let destination = tmp_dir.path().to_path_buf();
/// let mut exporter = Exporter::new(source, destination);
///
/// // add_postprocessor registers a new postprocessor. In this example we use a closure.
/// exporter.add_postprocessor(&|context, _events| {
///     // This is the key we'll insert into the frontmatter. In this case, the string "foo".
///     let key = Value::String("foo".to_string());
///     // This is the value we'll insert into the frontmatter. In this case, the string "bar".
///     let value = Value::String("baz".to_string());
///
///     // Frontmatter can be updated in-place, so we can call insert on it directly.
///     context.frontmatter.insert(key, value);
///
///     // This return value indicates processing should continue.
///     PostprocessorResult::Continue
/// });
///
/// exporter.run().unwrap();
/// ```
///
/// ## Change note contents
///
/// In this example a note's markdown content is changed by iterating over the [`MarkdownEvents`]
/// and changing the text when we encounter a [text element][Event::Text].
///
/// Instead of using a closure like above, this example shows how to use a separate function
/// definition.
/// ```
/// # use obsidian_export::{Context, Exporter, MarkdownEvents, PostprocessorResult};
/// # use pulldown_cmark::{CowStr, Event};
/// # use std::path::PathBuf;
/// # use tempfile::TempDir;
/// #
/// /// This postprocessor replaces any instance of "foo" with "bar" in the note body.
/// fn foo_to_bar(context: &mut Context, events: &mut MarkdownEvents) -> PostprocessorResult {
///     for event in events.iter_mut() {
///         if let Event::Text(text) = event {
///             *event = Event::Text(CowStr::from(text.replace("foo", "bar")))
///         }
///     }
///     PostprocessorResult::Continue
/// }
///
/// # let tmp_dir = TempDir::new().expect("failed to make tempdir");
/// # let source = PathBuf::from("tests/testdata/input/postprocessors");
/// # let destination = tmp_dir.path().to_path_buf();
/// # let mut exporter = Exporter::new(source, destination);
/// exporter.add_postprocessor(&foo_to_bar);
/// # exporter.run().unwrap();
/// ```
pub type Postprocessor<'f> =
    dyn Fn(&mut Context, &mut MarkdownEvents<'_>) -> PostprocessorResult + Send + Sync + 'f;
type Result<T, E = ExportError> = std::result::Result<T, E>;

const NOTE_RECURSION_LIMIT: usize = 10;

#[non_exhaustive]
#[derive(Debug, Snafu)]
/// `ExportError` represents all errors which may be returned when using this crate.
pub enum ExportError {
    #[snafu(display("failed to read from '{}'", path.display()))]
    /// This occurs when a read IO operation fails.
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("failed to write to '{}'", path.display()))]
    /// This occurs when a write IO operation fails.
    WriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Encountered an error while trying to walk '{}'", path.display()))]
    /// This occurs when an error is encountered while trying to walk a directory.
    WalkDirError {
        path: PathBuf,
        source: ignore::Error,
    },

    #[snafu(display("Failed to read the mtime of '{}'", path.display()))]
    /// This occurs when a file's modified time cannot be read
    ModTimeReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Failed to set the mtime of '{}'", path.display()))]
    /// This occurs when a file's modified time cannot be set
    ModTimeSetError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("No such file or directory: {}", path.display()))]
    /// This occurs when an operation is requested on a file or directory which does not exist.
    PathDoesNotExist { path: PathBuf },

    #[snafu(display("Invalid character encoding encountered"))]
    /// This error may occur when invalid UTF8 is encountered.
    ///
    /// Currently, operations which assume UTF8 perform lossy encoding however.
    CharacterEncodingError { source: str::Utf8Error },

    #[snafu(display("Recursion limit exceeded"))]
    /// This error occurs when embedded notes are too deeply nested or cause an infinite loop.
    ///
    /// When this happens, `file_tree` contains a list of all the files which were processed
    /// leading up to this error.
    RecursionLimitExceeded { file_tree: Vec<PathBuf> },

    #[snafu(display("Failed to export '{}'", path.display()))]
    /// This occurs when a file fails to export successfully.
    FileExportError {
        path: PathBuf,
        #[snafu(source(from(ExportError, Box::new)))]
        source: Box<ExportError>,
    },

    #[snafu(display("Failed to decode YAML frontmatter in '{}'", path.display()))]
    FrontMatterDecodeError {
        path: PathBuf,
        #[snafu(source(from(serde_yaml::Error, Box::new)))]
        source: Box<serde_yaml::Error>,
    },

    #[snafu(display("Failed to encode YAML frontmatter for '{}'", path.display()))]
    FrontMatterEncodeError {
        path: PathBuf,
        #[snafu(source(from(serde_yaml::Error, Box::new)))]
        source: Box<serde_yaml::Error>,
    },

    #[snafu(display("Section '{}' not found in '{}'", section, path.display()))]
    /// This occurs when an embed points at a section which doesn't exist in the target
    /// note and [`MissingSectionStrategy::Fail`] is configured.
    SectionNotFound { section: String, path: PathBuf },

    #[snafu(display(
        "start-at path '{}' is not under the export root '{}'",
        start_at.display(),
        root.display()
    ))]
    /// This occurs when [`Exporter::start_at`] points outside of the export root,
    /// which would otherwise silently export zero files.
    StartAtNotUnderRoot { start_at: PathBuf, root: PathBuf },

    #[snafu(display("Export completed with {} failing file(s)", errors.len()))]
    /// This occurs when one or more files failed to export and [`Exporter::fail_fast`]
    /// is disabled (the default). All other files will have been exported.
    ExportCompletedWithErrors { errors: Vec<FailedFile> },

    #[snafu(display(
        "failed to resolve the canonical path of '{}': {}",
        path.display(),
        source
    ))]
    /// This occurs when the filesystem cannot produce an absolute,
    /// normalized form of a path (e.g. permission issues, or a path that
    /// disappeared between validation and use).
    CanonicalizeError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display(
        "diagram tool '{}' required by renderer '{}' is unavailable: {}",
        tool,
        renderer,
        hint
    ))]
    /// This occurs when diagram rendering is enabled and a required external
    /// tool is neither explicitly configured nor found on `PATH`. Raised by
    /// the prescan in [`Exporter::run`], before any output file is written,
    /// so a missing tool fails the export atomically.
    DiagramToolNotFound {
        tool: String,
        renderer: String,
        hint: String,
    },
}

/// A single failed file, as reported by [`ExportError::ExportCompletedWithErrors`].
#[derive(Debug)]
#[non_exhaustive]
pub struct FailedFile {
    /// The source path of the file that failed to export.
    pub path: PathBuf,
    /// The error that caused the export of this file to fail.
    pub error: ExportError,
}

/// Events emitted during [`Exporter::run`], for progress reporting and structured
/// error/warning collection. Register a handler with [`Exporter::on_event`].
///
/// Callbacks are invoked from parallel worker threads and must be `Send + Sync`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ExportEvent {
    /// Emitted once before any file is processed. `total` is the number of files about
    /// to be processed.
    Start { total: usize },
    /// A file was exported successfully.
    FileDone { path: PathBuf },
    /// A file was skipped, e.g. by a postprocessor.
    FileSkipped { path: PathBuf },
    /// A file failed to export. Unless [`Exporter::fail_fast`] is enabled, the export
    /// continues with the remaining files.
    ///
    /// The error is provided in string form; the structured error of the aggregate
    /// result ([`ExportError::ExportCompletedWithErrors`]) retains full error types.
    FileFailed { path: PathBuf, message: String },
    /// A non-fatal warning, e.g. a wikilink pointing at a note that doesn't exist.
    /// `path` is the note the warning originates from, when known.
    Warning {
        path: Option<PathBuf>,
        message: String,
    },
    /// A diagram code block is about to be rendered through an external tool.
    ///
    /// `index` is 1-based within `total`, the number of renderable blocks
    /// found by the prescan. Rendering failures of individual blocks are
    /// reported separately as [`ExportEvent::Warning`], keeping the export
    /// itself going.
    DiagramRender {
        /// The fenced code block language, verbatim first word (e.g. `mermaid`).
        language: String,
        /// 1-based position within the run's total renderable blocks.
        index: usize,
        /// Total renderable blocks found by the prescan.
        total: usize,
    },
    /// Emitted once after processing stops. `failed` lists the source paths of all
    /// files that failed. Emitted on every termination of a started run — successful,
    /// with failures, or aborted by [`Exporter::fail_fast`] — so event consumers can
    /// rely on its presence; only its absence signals a hard crash of the process.
    End { failed: Vec<PathBuf> },
}

/// Emitted by [Postprocessor]s to signal the next action to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostprocessorResult {
    /// Continue with the next post-processor (if any).
    Continue,
    /// Use this note, but don't run any more post-processors after this one.
    StopHere,
    /// Skip this note (don't export it) and don't run any more post-processors.
    StopAndSkipNote,
}

/// Controls what happens when an embed points at a section which doesn't exist in the
/// target note (including block references like `![[note#^block-id]]`, which never match
/// a heading).
///
/// The strategy is applied independently at every level of embedding: a missing section
/// only affects that single embed, never the rest of the parent note. Detection of
/// embed cycles and the recursion limit are orthogonal and keep working as before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum MissingSectionStrategy {
    /// Embed the entire note (upstream behavior). The result silently contains more
    /// content than the reference asked for.
    EmbedFull,
    /// Replace the embed with nothing and emit a warning. Closest to Obsidian's own
    /// "not found" rendering. This is the default.
    #[default]
    Skip,
    /// Fail the export of the file containing the embed with [`ExportError::SectionNotFound`].
    Fail,
}

/// Callback receiving [`ExportEvent`]s during [`Exporter::run`]. Invoked from parallel
/// worker threads, hence the `Send + Sync` bound.
pub type ExportEventCallback = Arc<dyn Fn(&ExportEvent) + Send + Sync>;

#[derive(Clone)]
/// Exporter provides the main interface to this library.
///
/// Users are expected to create an Exporter using [`Exporter::new`], optionally followed by
/// customization using [`Exporter::frontmatter_strategy`] and [`Exporter::walk_options`].
///
/// After that, calling [`Exporter::run`] will start the export process.
pub struct Exporter<'a> {
    root: PathBuf,
    destination: PathBuf,
    start_at: PathBuf,
    frontmatter_strategy: FrontmatterStrategy,
    vault_contents: Option<Arc<[PathBuf]>>,
    vault_index: Option<VaultIndex>,
    walk_options: WalkOptions<'a>,
    process_embeds_recursively: bool,
    preserve_mtime: bool,
    missing_section_strategy: MissingSectionStrategy,
    fail_fast: bool,
    diagram_renderers: Vec<DiagramRenderer>,
    diagram_format: DiagramFormat,
    diagram_bins: BTreeMap<ToolName, PathBuf>,
    /// Populated by the prescan in [`Exporter::run`] when diagram rendering
    /// is enabled; shared (immutable, with atomics for progress) across
    /// worker threads.
    diagram_state: Option<Arc<DiagramState>>,
    event_callback: Option<ExportEventCallback>,
    postprocessors: Vec<&'a Postprocessor<'a>>,
    embed_postprocessors: Vec<&'a Postprocessor<'a>>,
}

impl fmt::Debug for Exporter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exporter")
            .field("root", &self.root)
            .field("destination", &self.destination)
            .field("frontmatter_strategy", &self.frontmatter_strategy)
            .field("vault_contents", &self.vault_contents)
            .field("walk_options", &self.walk_options)
            .field(
                "process_embeds_recursively",
                &self.process_embeds_recursively,
            )
            .field("missing_section_strategy", &self.missing_section_strategy)
            .field("fail_fast", &self.fail_fast)
            .field("diagram_renderers", &self.diagram_renderers)
            .field("diagram_format", &self.diagram_format)
            .field("diagram_bins", &self.diagram_bins)
            .field(
                "event_callback",
                &match self.event_callback {
                    Some(_) => "<set>",
                    None => "<not set>",
                },
            )
            .field("preserve_mtime", &self.preserve_mtime)
            .field(
                "postprocessors",
                &format!("<{} postprocessors active>", self.postprocessors.len()),
            )
            .field(
                "embed_postprocessors",
                &format!(
                    "<{} postprocessors active>",
                    self.embed_postprocessors.len()
                ),
            )
            .finish()
    }
}

impl<'a> Exporter<'a> {
    /// Create a new exporter which reads notes from `root` and exports these to
    /// `destination`.
    #[must_use]
    pub fn new(root: PathBuf, destination: PathBuf) -> Self {
        Self {
            start_at: root.clone(),
            root,
            destination,
            frontmatter_strategy: FrontmatterStrategy::Auto,
            walk_options: WalkOptions::default(),
            process_embeds_recursively: true,
            preserve_mtime: false,
            missing_section_strategy: MissingSectionStrategy::default(),
            fail_fast: false,
            diagram_renderers: vec![],
            diagram_format: DiagramFormat::Svg,
            diagram_bins: BTreeMap::new(),
            diagram_state: None,
            event_callback: None,
            vault_contents: None,
            vault_index: None,
            postprocessors: vec![],
            embed_postprocessors: vec![],
        }
    }

    /// Set a custom starting point for the export.
    ///
    /// Normally all notes under `root` (except for notes excluded by ignore rules) will be
    /// exported. When `start_at` is set, only notes under this path will be exported to the
    /// target destination.
    pub fn start_at(&mut self, start_at: PathBuf) -> &mut Self {
        self.start_at = start_at;
        self
    }

    /// Set the [`WalkOptions`] to be used for this exporter.
    pub const fn walk_options(&mut self, options: WalkOptions<'a>) -> &mut Self {
        self.walk_options = options;
        self
    }

    /// Set the [`FrontmatterStrategy`] to be used for this exporter.
    pub const fn frontmatter_strategy(&mut self, strategy: FrontmatterStrategy) -> &mut Self {
        self.frontmatter_strategy = strategy;
        self
    }

    /// Set the behavior when recursive embeds are encountered.
    ///
    /// When `recursive` is true (the default), emdeds are always processed recursively. This may
    /// lead to infinite recursion when note A embeds B, but B also embeds A.
    /// (When this happens, [`ExportError::RecursionLimitExceeded`] will be returned by
    /// [`Exporter::run`]).
    ///
    /// When `recursive` is false, if a note is encountered for a second time while processing the
    /// original note, instead of embedding it again a link to the note is inserted instead.
    pub const fn process_embeds_recursively(&mut self, recursive: bool) -> &mut Self {
        self.process_embeds_recursively = recursive;
        self
    }

    /// Set whether the modified time of exported files should be preserved.
    ///
    /// When `preserve` is true, the modified time of exported files will be set to the modified
    /// time of the source file.
    pub const fn preserve_mtime(&mut self, preserve: bool) -> &mut Self {
        self.preserve_mtime = preserve;
        self
    }

    /// Set the strategy for embeds pointing at a missing section (default:
    /// [`MissingSectionStrategy::Skip`]).
    pub const fn missing_section_strategy(
        &mut self,
        strategy: MissingSectionStrategy,
    ) -> &mut Self {
        self.missing_section_strategy = strategy;
        self
    }

    /// Set whether the export should stop at the first failing file (default: false).
    ///
    /// When disabled (the default), a failing file is recorded and the export continues
    /// with the remaining files; [`Exporter::run`] then returns
    /// [`ExportError::ExportCompletedWithErrors`] listing every failure. When enabled,
    /// the first error stops the run: no new files are scheduled, though files already
    /// being processed concurrently may still complete (and be reported as usual).
    pub const fn fail_fast(&mut self, fail_fast: bool) -> &mut Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Enable rendering of diagram code blocks through external tools
    /// (default: none).
    ///
    /// For each enabled renderer, the tools it requires must be resolvable —
    /// through [`Exporter::diagram_bins`] or a `PATH` scan — whenever a
    /// language it covers occurs in the vault. [`Exporter::run`] verifies
    /// this in a prescan before any output file is written, failing
    /// atomically with [`ExportError::DiagramToolNotFound`] otherwise.
    pub fn diagram_renderers(&mut self, renderers: Vec<DiagramRenderer>) -> &mut Self {
        self.diagram_renderers = renderers;
        self
    }

    /// Set the output format for rendered diagrams (default:
    /// [`DiagramFormat::Svg`]). Renderers without raster output fall back to
    /// SVG and emit a warning.
    pub const fn diagram_format(&mut self, format: DiagramFormat) -> &mut Self {
        self.diagram_format = format;
        self
    }

    /// Set explicit executable paths for external diagram tools, overriding
    /// `PATH` lookup (default: none). Tool keys are the canonical executable
    /// names (`dot`, `mmdc`, `wavedrom`, `latex`, `dvisvgm`).
    pub fn diagram_bins(&mut self, bins: BTreeMap<ToolName, PathBuf>) -> &mut Self {
        self.diagram_bins = bins;
        self
    }

    /// Register a callback receiving [`ExportEvent`]s during [`Exporter::run`].
    ///
    /// The callback is invoked from parallel worker threads and must therefore be
    /// `Send + Sync`. Only one callback can be registered; a subsequent call replaces
    /// the previous one. Without a callback, warnings are printed to stderr instead.
    pub fn on_event(&mut self, callback: ExportEventCallback) -> &mut Self {
        self.event_callback = Some(callback);
        self
    }

    /// Append a function to the chain of [postprocessors][Postprocessor] to run on exported
    /// Obsidian Markdown notes.
    pub fn add_postprocessor(&mut self, processor: &'a Postprocessor<'_>) -> &mut Self {
        self.postprocessors.push(processor);
        self
    }

    /// Append a function to the chain of [postprocessors][Postprocessor] for embeds.
    pub fn add_embed_postprocessor(&mut self, processor: &'a Postprocessor<'_>) -> &mut Self {
        self.embed_postprocessors.push(processor);
        self
    }

    /// Export notes using the settings configured on this exporter.
    #[allow(clippy::too_many_lines)]
    pub fn run(&mut self) -> Result<()> {
        if !self.root.exists() {
            return Err(ExportError::PathDoesNotExist {
                path: self.root.clone(),
            });
        }

        // A start_at outside of root would silently produce an empty export; reject it
        // up front so users get a clear error instead.
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

        let contents: Arc<[PathBuf]> = Arc::from(vault_contents(
            self.root.as_path(),
            self.walk_options.clone(),
        )?);
        // Prebuild the suffix index so wikilink resolution is O(1) per reference instead
        // of a linear scan over the whole vault.
        self.vault_index = Some(VaultIndex::build(&contents));
        self.vault_contents = Some(contents);

        // Diagram rendering prescan: count the renderable blocks actually
        // present and resolve every external tool they need. Runs before any
        // output file is written so a missing tool fails the export
        // atomically, leaving the destination untouched.
        if !self.diagram_renderers.is_empty() {
            self.prepare_diagram_state()?;
        }

        // When a single file is specified, just need to export that specific file instead of
        // iterating over all discovered files. This also allows us to accept destination as either
        // a file or a directory name.
        if self.root.is_file() || self.start_at.is_file() {
            let source_filename = self
                .start_at
                .file_name()
                .expect("File without a filename? How is that possible?")
                .to_string_lossy();

            let destination = match self.destination.is_dir() {
                true => self.destination.join(String::from(source_filename)),
                false => {
                    // Avoid recursively creating self.destination through the call to
                    // export_note when the parent directory doesn't exist.
                    validate_destination_parent(&self.destination)?;
                    self.destination.clone()
                }
            };
            self.emit(&ExportEvent::Start { total: 1 });
            let result = self.export_note(&self.start_at, &destination);
            match result {
                Ok(true) => {
                    self.emit(&ExportEvent::FileDone {
                        path: self.start_at.clone(),
                    });
                }
                Ok(false) => {
                    self.emit(&ExportEvent::FileSkipped {
                        path: self.start_at.clone(),
                    });
                }
                Err(error) => {
                    self.emit(&ExportEvent::FileFailed {
                        path: self.start_at.clone(),
                        message: error_chain_string(&error),
                    });
                    // The stream contract requires an end event on every started run,
                    // so consumers can distinguish "run finished (with errors)" from
                    // "process died".
                    self.emit(&ExportEvent::End {
                        failed: vec![self.start_at.clone()],
                    });
                    return Err(error);
                }
            }
            self.emit(&ExportEvent::End { failed: vec![] });
            return Ok(());
        }

        if !self.destination.exists() {
            return Err(ExportError::PathDoesNotExist {
                path: self.destination.clone(),
            });
        }

        let files: Vec<PathBuf> = self
            .vault_contents
            .as_ref()
            .expect("vault_contents is always populated by run() before iterating")
            .iter()
            .filter(|file| file.starts_with(&self.start_at))
            .cloned()
            .collect();
        self.emit(&ExportEvent::Start { total: files.len() });

        if self.fail_fast {
            // Even though try_for_each short-circuits, files already in flight on other
            // worker threads may fail around the same time; collect them all so the end
            // event can report what actually failed.
            let failed_paths: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
            let result = files.into_par_iter().try_for_each(|file| {
                let relative_path = file
                    .strip_prefix(&self.start_at)
                    .expect("file should always be nested under root")
                    .to_path_buf();
                let destination = &self.destination.join(relative_path);
                match self.export_note(&file, destination) {
                    Ok(true) => {
                        self.emit(&ExportEvent::FileDone { path: file });
                        Ok(())
                    }
                    Ok(false) => {
                        self.emit(&ExportEvent::FileSkipped { path: file });
                        Ok(())
                    }
                    Err(error) => {
                        self.emit(&ExportEvent::FileFailed {
                            path: file.clone(),
                            message: error_chain_string(&error),
                        });
                        failed_paths
                            .lock()
                            .expect("fail-fast failure collector mutex poisoned")
                            .push(file);
                        Err(error)
                    }
                }
            });
            let failed_paths = failed_paths
                .into_inner()
                .expect("fail-fast failure collector mutex poisoned");
            // Emitted on aborts as well: see the stream contract on ExportEvent::End.
            self.emit(&ExportEvent::End {
                failed: failed_paths,
            });
            return result;
        }

        let failures: Mutex<Vec<FailedFile>> = Mutex::new(Vec::new());
        files.into_par_iter().for_each(|file| {
            let relative_path = file
                .strip_prefix(&self.start_at)
                .expect("file should always be nested under root")
                .to_path_buf();
            let destination = &self.destination.join(relative_path);
            match self.export_note(&file, destination) {
                Ok(true) => {
                    self.emit(&ExportEvent::FileDone { path: file });
                }
                Ok(false) => {
                    self.emit(&ExportEvent::FileSkipped { path: file });
                }
                Err(error) => {
                    self.emit(&ExportEvent::FileFailed {
                        path: file.clone(),
                        message: error_chain_string(&error),
                    });
                    failures
                        .lock()
                        .expect("failure collector mutex poisoned")
                        .push(FailedFile { path: file, error });
                }
            }
        });

        let failures = failures
            .into_inner()
            .expect("failure collector mutex poisoned");
        let failed_paths = failures.iter().map(|f| f.path.clone()).collect();
        self.emit(&ExportEvent::End {
            failed: failed_paths,
        });
        if failures.is_empty() {
            Ok(())
        } else {
            Err(ExportError::ExportCompletedWithErrors { errors: failures })
        }
    }

    /// Count renderable diagram blocks across the export file set and
    /// resolve every external tool they need, populating `diagram_state`.
    ///
    /// Runs inside [`Exporter::run`] before the first output file is
    /// written, so an unresolvable tool aborts the export atomically. The
    /// walk honors the same `start_at` filter as the export itself; tag
    /// filtering is deliberately not simulated (notes skipped by
    /// `StopAndSkipNote` still count), erring on the stricter side. Files
    /// that cannot be read are skipped here — the main pass reports them as
    /// per-file failures, which keeps a prescan error from failing the
    /// whole export.
    #[allow(clippy::arithmetic_side_effects)]
    fn prepare_diagram_state(&mut self) -> Result<()> {
        let contents = self
            .vault_contents
            .as_ref()
            .expect("vault_contents is always populated by run() before the diagram prescan");

        let mut total = 0;
        let mut renderers_needed: BTreeSet<DiagramRenderer> = BTreeSet::new();
        for file in contents
            .iter()
            .filter(|file| file.starts_with(&self.start_at) && is_markdown_file(file))
        {
            // A file the prescan cannot read (IO error, non-UTF-8) is
            // reported as a per-file failure by the main pass; skipping it
            // here keeps the prescan as forgiving as the export itself
            // instead of failing the whole run up front.
            let Ok(text) = fs::read_to_string(file) else {
                continue;
            };
            let hit = diagrams::prescan_note(&text, &self.diagram_renderers);
            total += hit.renderable_blocks;
            renderers_needed.extend(hit.renderers);
        }

        let toolset = DiagramToolset::resolve(
            &renderers_needed.into_iter().collect::<Vec<_>>(),
            &self.diagram_bins,
        )
        .map_err(|error| match error {
            ToolResolutionError::ExplicitMissing { tool, path } => {
                ExportError::DiagramToolNotFound {
                    tool: tool.as_str().into(),
                    renderer: tool.primary_renderer().name().into(),
                    hint: format!("explicit path '{}' does not exist", path.display()),
                }
            }
            ToolResolutionError::NotFoundOnPath { tool, hint } => {
                ExportError::DiagramToolNotFound {
                    tool: tool.as_str().into(),
                    renderer: tool.primary_renderer().name().into(),
                    hint: hint.into(),
                }
            }
        })?;

        self.diagram_state = Some(Arc::new(DiagramState::new(
            toolset,
            self.diagram_format,
            self.diagram_renderers.clone(),
            total,
        )));
        Ok(())
    }

    /// Resolve a reference string to a vault file via the prebuilt index.
    fn resolve_reference(&self, file: &str, context: &Context) -> Option<&PathBuf> {
        let index = self
            .vault_index
            .as_ref()
            .expect("vault_index is always built by run() before exporting");
        if let Some(found) = index.lookup(file) {
            return Some(found);
        }

        // Obsidian resolves wikilinks containing explicit relative components (`./`,
        // `../`) against the containing note's directory rather than by vault-wide
        // suffix match. The index never contains such components, so re-resolve the
        // reference against the note's location and look up the normalized result.
        if !file.split(['/', '\\']).any(|c| c == "." || c == "..") {
            return None;
        }
        let base = context.current_file().parent()?;
        let resolved = normalize_lexically(&base.join(file));
        index.lookup(&resolved.to_string_lossy())
    }

    fn emit(&self, event: &ExportEvent) {
        if let Some(callback) = &self.event_callback {
            callback(event);
        }
    }

    fn warn(&self, source: Option<&Path>, message: String) {
        match &self.event_callback {
            Some(callback) => callback(&ExportEvent::Warning {
                path: source.map(Path::to_path_buf),
                message,
            }),
            None => eprintln!("Warning: {message}"),
        }
    }
    #[allow(clippy::shadow_unrelated)]
    fn export_note(&self, src: &Path, dest: &Path) -> Result<bool> {
        let output_file = match is_markdown_file(src) {
            true => self.parse_and_export_obsidian_note(src, dest),
            false => copy_file(src, dest),
        }
        .context(FileExportSnafu { path: src })?;

        // Don't try to set mtime if the file was not exported
        if let Some(dest) = &output_file {
            if self.preserve_mtime {
                copy_mtime(src, dest).context(FileExportSnafu { path: src })?;
            }
        }

        Ok(output_file.is_some())
    }

    /// Parse an Obsidian note and export it to the destination path, applying
    /// any configured postprocessors in the process.
    ///
    /// Because postprocessors may alter the destination path or prevent a note
    /// from being exported at all, the inner `<Option<PathBuf>>` is used to
    /// indicate whether the note was exported at all, and where.
    fn parse_and_export_obsidian_note(&self, src: &Path, dest: &Path) -> Result<Option<PathBuf>> {
        let mut context = Context::new(src.to_path_buf(), dest.to_path_buf());

        let (frontmatter, mut markdown_events) = self.parse_obsidian_note(src, &context)?;
        context.frontmatter = frontmatter;
        for func in &self.postprocessors {
            match func(&mut context, &mut markdown_events) {
                PostprocessorResult::StopHere => break,
                PostprocessorResult::StopAndSkipNote => return Ok(None),
                PostprocessorResult::Continue => (),
            }
        }

        // Diagram rendering is a built-in final stage rather than a
        // postprocessor: it needs Exporter-owned state (resolved tools, the
        // event callback) and runs after user postprocessors, so those keep
        // seeing the original code blocks.
        if let Some(state) = &self.diagram_state {
            diagrams::process_diagram_events(
                state,
                &context,
                &mut markdown_events,
                &|event: &ExportEvent| self.emit(event),
                &|message: String| self.warn(None, message),
            );
        }

        let mut outfile = create_file(&context.destination)?;
        let write_frontmatter = match self.frontmatter_strategy {
            FrontmatterStrategy::Always => true,
            FrontmatterStrategy::Never => false,
            FrontmatterStrategy::Auto => !context.frontmatter.is_empty(),
        };
        if write_frontmatter {
            let mut frontmatter_str = frontmatter_to_str(&context.frontmatter)
                .context(FrontMatterEncodeSnafu { path: src })?;
            frontmatter_str.push('\n');
            outfile
                .write_all(frontmatter_str.as_bytes())
                .context(WriteSnafu {
                    path: &context.destination,
                })?;
        }
        outfile
            .write_all(render_mdevents_to_mdtext(&markdown_events).as_bytes())
            .context(WriteSnafu {
                path: &context.destination,
            })?;
        Ok(Some(context.destination))
    }

    fn parse_obsidian_note<'b>(
        &self,
        path: &Path,
        context: &Context,
    ) -> Result<(Frontmatter, MarkdownEvents<'b>)> {
        let (frontmatter, events) = Self::parse_raw_note(path)?;
        let events = self.expand_references(&events, context)?;
        Ok((frontmatter, events))
    }

    /// Parse a note into raw markdown events: frontmatter stripped, references
    /// (`[[...]]` / `![[...]]`) normalized to a canonical five-event form
    /// (`![`/`[`, `[`, single text event, `]`, `]`) with the reference text
    /// taken verbatim from the source. The verbatim slice is what preserves
    /// spellings like `__bold__` that pulldown-cmark would otherwise consume
    /// as formatting events.
    ///
    /// Splitting raw parsing from reference expansion is what allows section
    /// cuts to run on a note's own events (see [`Exporter::embed_file`]):
    /// headings pulled in by nested embeds must not terminate an outer
    /// section cut.
    #[allow(clippy::shadow_unrelated)]
    #[allow(clippy::too_many_lines)]
    fn parse_raw_note<'b>(path: &Path) -> Result<(Frontmatter, MarkdownEvents<'b>)> {
        Self::parse_raw_note_with_refs(path)
            .map(|(frontmatter, events, _refs)| (frontmatter, events))
    }

    /// Like [`parse_raw_note`], additionally returning every reference
    /// recognized during the scan with the byte offset of its verbatim text
    /// in the source. The link checker uses the offsets to attribute each
    /// reference to a source line; the export path ignores them.
    #[allow(clippy::shadow_unrelated)]
    #[allow(clippy::too_many_lines)]
    fn parse_raw_note_with_refs<'b>(
        path: &Path,
    ) -> Result<(Frontmatter, MarkdownEvents<'b>, Vec<RawNoteRef>)> {
        let content = fs::read_to_string(path).context(ReadSnafu { path })?;
        let mut frontmatter = String::new();

        let parser_options = markdown_parser_options();

        let mut events: MarkdownEvents<'b> = vec![];
        let mut ref_parser = RefParser::new();
        // References recognized so far, in source order, for callers (the
        // link checker) that need the verbatim text plus source offsets.
        let mut refs: Vec<RawNoteRef> = vec![];
        // Events of the reference currently being scanned, flushed verbatim
        // when the scan resets, or collapsed into the canonical form once the
        // reference completes.
        let mut buffer: Vec<Event<'_>> = vec![];
        // Source offsets of the reference text: end of the second `[`, start
        // of the first `]`.
        let (mut ref_start, mut ref_end): (Option<usize>, Option<usize>) = (None, None);

        let mut parser = Parser::new_ext(&content, parser_options).into_offset_iter();
        // When encountering a metadata block (frontmatter), collect all events until getting
        // to the end of the block, at which point the nested loop will break out to the outer
        // loop again.
        'outer: while let Some((event, range)) = parser.next() {
            if matches!(event, Event::Start(Tag::MetadataBlock(_kind))) {
                for (event, _range) in parser.by_ref() {
                    match event {
                        Event::Text(cowstr) => frontmatter.push_str(&cowstr),
                        Event::End(TagEnd::MetadataBlock(_kind)) => {
                            continue 'outer;
                        }
                        // Anything else inside a metadata block is unexpected, but skipping it
                        // beats panicking inside a rayon worker thread (which would abort the
                        // entire export process).
                        _ => (),
                    }
                }
            }
            if ref_parser.state == RefParserState::Resetting {
                events.extend(std::mem::take(&mut buffer).into_iter().map(event_to_owned));
                ref_parser.reset();
                ref_start = None;
                ref_end = None;
            }
            let text_is =
                |literal: &str| matches!(&event, Event::Text(text) if text.as_ref() == literal);
            if ref_parser.state == RefParserState::ExpectSecondOpenBracket && text_is("[") {
                ref_start = Some(range.end);
            }
            if ref_parser.state == RefParserState::ExpectRefTextOrCloseBracket && text_is("]") {
                ref_end = Some(range.start);
            }
            buffer.push(event.clone());
            match ref_parser.state {
                RefParserState::NoState => {
                    if text_is("![") {
                        ref_parser.ref_type = Some(RefType::Embed);
                        ref_parser.transition(RefParserState::ExpectSecondOpenBracket);
                    } else if text_is("[") {
                        ref_parser.ref_type = Some(RefType::Link);
                        ref_parser.transition(RefParserState::ExpectSecondOpenBracket);
                    } else {
                        events.push(event_to_owned(event));
                        buffer.clear();
                    }
                }
                RefParserState::ExpectSecondOpenBracket => {
                    if text_is("[") {
                        ref_parser.transition(RefParserState::ExpectRefText);
                    } else {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                }
                RefParserState::ExpectRefText => match &event {
                    Event::Text(text) if text.as_ref() == "]" => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                    // Formatting events (Strong/Emphasis/Strikethrough) carry
                    // no literal text of their own; the verbatim source slice
                    // below recovers their original spelling.
                    Event::Text(_)
                    | Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough)
                    | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => {
                        ref_parser.transition(RefParserState::ExpectRefTextOrCloseBracket);
                    }
                    _ => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                },
                RefParserState::ExpectRefTextOrCloseBracket => match &event {
                    Event::Text(text) if text.as_ref() == "]" => {
                        ref_parser.transition(RefParserState::ExpectFinalCloseBracket);
                    }
                    Event::Text(_)
                    | Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough)
                    | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => (),
                    _ => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                },
                RefParserState::ExpectFinalCloseBracket => {
                    if text_is("]") {
                        // Collapse the scanned events into the canonical form
                        // with the reference text sliced verbatim from the
                        // source.
                        let opener = match ref_parser.ref_type {
                            Some(RefType::Embed) => "![",
                            _ => "[",
                        };
                        let literal = match (ref_start, ref_end) {
                            (Some(start), Some(end)) => content
                                .get(start..end)
                                .expect("reference offsets are inside the source"),
                            _ => "",
                        };
                        if let Some(start) = ref_start {
                            refs.push(RawNoteRef {
                                embed: matches!(ref_parser.ref_type, Some(RefType::Embed)),
                                text: literal.to_owned(),
                                start,
                            });
                        }
                        for text in [opener, "[", literal, "]", "]"] {
                            events.push(Event::Text(CowStr::from(text.to_owned())));
                        }
                        buffer.clear();
                        ref_start = None;
                        ref_end = None;
                    }
                    // Both the collapsed reference and an invalid terminator
                    // reset the scanner.
                    ref_parser.transition(RefParserState::Resetting);
                }
                // Resetting is handled at the top of the loop; recovering by
                // resetting is safer than panicking.
                RefParserState::Resetting => ref_parser.reset(),
            }
        }
        if !buffer.is_empty() {
            events.extend(std::mem::take(&mut buffer).into_iter().map(event_to_owned));
        }

        Ok((
            frontmatter_from_str(&frontmatter).context(FrontMatterDecodeSnafu { path })?,
            events,
            refs,
        ))
    }

    /// Expand Obsidian references in an owned event stream: wikilinks become
    /// markdown links, embeds recursively splice in (and section-cut) the
    /// referenced notes.
    ///
    /// Matching compares text contents rather than the specific `CowStr`
    /// variant because the input events are owned (`event_to_owned` output),
    /// never `CowStr::Borrowed`.
    #[allow(clippy::too_many_lines)]
    fn expand_references<'b>(
        &self,
        events: &[Event<'b>],
        context: &Context,
    ) -> Result<MarkdownEvents<'b>> {
        let mut ref_parser = RefParser::new();
        let mut events_out = vec![];
        // Most of the time, a reference triggers 5 events: [ or ![, [, <text>, ], ]
        let mut buffer = Vec::with_capacity(5);

        // The input is borrowed: same-file embeds below need the full input
        // stream to locate their target section or block.
        for event in events {
            if ref_parser.state == RefParserState::Resetting {
                events_out.append(&mut buffer);
                buffer.clear();
                ref_parser.reset();
            }
            buffer.push(event.clone());
            match ref_parser.state {
                RefParserState::NoState => match event {
                    Event::Text(text) if text.as_ref() == "![" => {
                        ref_parser.ref_type = Some(RefType::Embed);
                        ref_parser.transition(RefParserState::ExpectSecondOpenBracket);
                    }
                    Event::Text(text) if text.as_ref() == "[" => {
                        ref_parser.ref_type = Some(RefType::Link);
                        ref_parser.transition(RefParserState::ExpectSecondOpenBracket);
                    }
                    _ => {
                        events_out.push(event.clone());
                        buffer.clear();
                    }
                },
                RefParserState::ExpectSecondOpenBracket => match event {
                    Event::Text(text) if text.as_ref() == "[" => {
                        ref_parser.transition(RefParserState::ExpectRefText);
                    }
                    _ => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                },
                RefParserState::ExpectRefText => match event {
                    Event::Text(text) if text.as_ref() == "]" => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                    Event::Text(text) => {
                        ref_parser.ref_text.push_str(text);
                        ref_parser.transition(RefParserState::ExpectRefTextOrCloseBracket);
                    }
                    Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => {
                        ref_parser.ref_text.push('*');
                        ref_parser.transition(RefParserState::ExpectRefTextOrCloseBracket);
                    }
                    Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => {
                        ref_parser.ref_text.push_str("**");
                        ref_parser.transition(RefParserState::ExpectRefTextOrCloseBracket);
                    }
                    Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough) => {
                        ref_parser.ref_text.push_str("~~");
                        ref_parser.transition(RefParserState::ExpectRefTextOrCloseBracket);
                    }
                    _ => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                },
                RefParserState::ExpectRefTextOrCloseBracket => match event {
                    Event::Text(text) if text.as_ref() == "]" => {
                        ref_parser.transition(RefParserState::ExpectFinalCloseBracket);
                    }
                    Event::Text(text) => {
                        ref_parser.ref_text.push_str(text);
                    }
                    Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => {
                        ref_parser.ref_text.push('*');
                    }
                    Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => {
                        ref_parser.ref_text.push_str("**");
                    }
                    Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough) => {
                        ref_parser.ref_text.push_str("~~");
                    }
                    _ => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                },
                RefParserState::ExpectFinalCloseBracket => match event {
                    Event::Text(text) if text.as_ref() == "]" => match ref_parser.ref_type {
                        Some(RefType::Link) => {
                            let mut elements = self.make_link_to_file(
                                ObsidianNoteReference::from_str(
                                    ref_parser.ref_text.clone().as_ref(),
                                ),
                                context,
                            );
                            events_out.append(&mut elements);
                            buffer.clear();
                            ref_parser.transition(RefParserState::Resetting);
                        }
                        Some(RefType::Embed) => {
                            let ref_text = ref_parser.ref_text.clone();
                            let note_ref = ObsidianNoteReference::from_str(ref_text.as_str());
                            let mut elements = match note_ref.file {
                                // A section or block of the note currently being
                                // expanded (`![[#Heading]]` / `![[#^block-id]]`).
                                None => self.embed_same_file(events, note_ref, context)?,
                                Some(_) => self.embed_file(ref_text.as_str(), context)?,
                            };
                            events_out.append(&mut elements);
                            buffer.clear();
                            ref_parser.transition(RefParserState::Resetting);
                        }
                        // A None ref_type here is a state machine invariant violation; bail out
                        // of the reference safely instead of panicking in a rayon worker.
                        None => ref_parser.transition(RefParserState::Resetting),
                    },
                    _ => {
                        ref_parser.transition(RefParserState::Resetting);
                    }
                },
                // Resetting is normally handled at the top of the loop; if it ever reaches
                // this point, recovering by resetting is safer than panicking.
                RefParserState::Resetting => ref_parser.reset(),
            }
        }
        if !buffer.is_empty() {
            events_out.append(&mut buffer);
        }

        Ok(events_out)
    }

    // Generate markdown elements for a file that is embedded within another note.
    //
    // - If the file being embedded is a note, it's content is included at the point of embed.
    // - If the file is an image, an image tag is generated.
    // - For other types of file, a regular link is created instead.
    #[allow(clippy::too_many_lines)]
    fn embed_file<'b>(
        &self,
        link_text: &'a str,
        context: &'a Context,
    ) -> Result<MarkdownEvents<'b>> {
        let note_ref = ObsidianNoteReference::from_str(link_text);

        let path = match note_ref.file {
            Some(file) => self.resolve_reference(file, context),

            // If we have None file it is either to a section or id within the same file and thus
            // the current embed logic will fail, recurssing until it reaches it's limit.
            // For now we just bail early.
            None => return Ok(self.make_link_to_file(note_ref, context)),
        };

        if path.is_none() {
            let current_file = context.current_file().to_string_lossy();
            self.warn(
                Some(context.current_file()),
                format!(
                    "Unable to find embedded note\n\tReference: '{}'\n\tSource: '{}'\n",
                    note_ref.file.unwrap_or_else(|| current_file.as_ref()),
                    context.current_file().display(),
                ),
            );
            return Ok(vec![]);
        }

        let path = path.unwrap();
        let mut child_context = Context::from_parent(context, path);
        // Recursion guard at the common embed entry point: every embed level
        // (cross-file or same-file) grows the file tree, so cyclic references
        // bottom out here instead of looping forever.
        if child_context.note_depth() > NOTE_RECURSION_LIMIT {
            return Err(ExportError::RecursionLimitExceeded {
                file_tree: child_context.file_tree(),
            });
        }
        let no_ext = OsString::new();

        if !self.process_embeds_recursively && context.file_tree().contains(path) {
            return Ok([
                vec![Event::Text(CowStr::Borrowed("→ "))],
                self.make_link_to_file(note_ref, &child_context),
            ]
            .concat());
        }

        let events = match path.extension().unwrap_or(&no_ext).to_str() {
            Some("md") => {
                // The section cut runs on the note's own raw events, before its
                // nested embeds expand: a heading injected by an inner embed must
                // not terminate the outer cut. Postprocessors below still see the
                // fully expanded content (see the Postprocessor docs).
                let (frontmatter, mut events) = Self::parse_raw_note(path)?;
                child_context.frontmatter = frontmatter;
                if let Some(section) = note_ref.section {
                    // Block references (`#^block-id`) locate the marked block;
                    // heading references cut the section. Both fall back to the
                    // missing-section strategy when they can't resolve.
                    let located = section.strip_prefix('^').map_or_else(
                        || reduce_to_section(&events, section),
                        |block_id| reduce_to_block(&events, block_id),
                    );
                    match located {
                        Some(reduced) => events = reduced,
                        None => match self.missing_section_strategy {
                            MissingSectionStrategy::EmbedFull => (),
                            MissingSectionStrategy::Skip => {
                                self.warn(
                                    Some(context.current_file()),
                                    format!(
                                        "Unable to find section '{section}' in note '{}'\n\tSource: '{}'\n",
                                        path.display(),
                                        context.current_file().display(),
                                    ),
                                );
                                events = vec![];
                            }
                            MissingSectionStrategy::Fail => {
                                return Err(ExportError::SectionNotFound {
                                    section: section.to_owned(),
                                    path: path.clone(),
                                });
                            }
                        },
                    }
                }
                let mut events = self.expand_references(&events, &child_context)?;
                for func in &self.embed_postprocessors {
                    // Postprocessors running on embeds shouldn't be able to change frontmatter (or
                    // any other metadata), so we give them a clone of the context.
                    match func(&mut child_context, &mut events) {
                        PostprocessorResult::StopHere => break,
                        PostprocessorResult::StopAndSkipNote => {
                            events = vec![];
                        }
                        PostprocessorResult::Continue => (),
                    }
                }
                events
            }
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => {
                // Obsidian's size syntax (`![[img.png|300]]`) surfaces as a purely numeric
                // label. Plain Markdown has no notion of image sizes, so fall back to the
                // filename instead of rendering a bare number as alt text.
                let note_ref = match note_ref.label {
                    Some(label)
                        if !label.is_empty() && label.chars().all(|c| c.is_ascii_digit()) =>
                    {
                        ObsidianNoteReference {
                            label: None,
                            ..note_ref
                        }
                    }
                    _ => note_ref,
                };
                self.make_link_to_file(note_ref, &child_context)
                    .into_iter()
                    .map(|event| match event {
                        // make_link_to_file returns a link to a file. With this we turn the link
                        // into an image reference instead. Slightly hacky, but avoids needing
                        // to keep another utility function around for this, or introducing an
                        // extra parameter on make_link_to_file.
                        Event::Start(Tag::Link {
                            link_type,
                            dest_url,
                            title,
                            id,
                        }) => Event::Start(Tag::Image {
                            link_type,
                            dest_url: CowStr::from(dest_url.into_string()),
                            title: CowStr::from(title.into_string()),
                            id: CowStr::from(id.into_string()),
                        }),
                        Event::End(TagEnd::Link) => Event::End(TagEnd::Image),
                        _ => event,
                    })
                    .collect()
            }
            _ => self.make_link_to_file(note_ref, &child_context),
        };
        Ok(events)
    }

    /// Embed a section or block of the note currently being expanded
    /// (`![[#Heading]]` / `![[#^block-id]]`).
    ///
    /// The current note's raw events are sliced directly. Every same-file
    /// embed level pushes the current file onto the file tree, so nesting
    /// (including a block embedding itself) is bounded by
    /// `NOTE_RECURSION_LIMIT` at this common embed entry point.
    fn embed_same_file<'b>(
        &self,
        events: &[Event<'b>],
        note_ref: ObsidianNoteReference<'_>,
        context: &Context,
    ) -> Result<MarkdownEvents<'b>> {
        let Some(section) = note_ref.section else {
            // Degenerate `![[#]]`-style refs carry no target; render as a link.
            return Ok(self.make_link_to_file(note_ref, context));
        };
        // A same-file embed always refers to the current file, which is
        // already on the file tree: under --no-recursive-embeds it degrades to
        // a link just like any other cyclic reference. So does an embed that
        // appears inside an expansion of the same file (e.g. a section whose
        // content embeds itself): expanding again would recurse into the same
        // target forever and bottom out with a misleading recursion error.
        if !self.process_embeds_recursively
            || context
                .file_tree()
                .iter()
                .filter(|p| *p == context.current_file())
                .count()
                > 1
        {
            return Ok([
                vec![Event::Text(CowStr::Borrowed("→ "))],
                self.make_link_to_file(note_ref, context),
            ]
            .concat());
        }
        let mut child_context = Context::from_parent(context, context.current_file());
        if child_context.note_depth() > NOTE_RECURSION_LIMIT {
            return Err(ExportError::RecursionLimitExceeded {
                file_tree: child_context.file_tree(),
            });
        }
        let located = section.strip_prefix('^').map_or_else(
            || reduce_to_section(events, section),
            |block_id| reduce_to_block(events, block_id),
        );
        // `events` is whatever slice this embed was reached through: at the
        // top level it is the whole file, but a same-file ref inside a
        // cross-file embed (`![[note#S]]` → slice → `![[#Other]]`) only sees
        // the slice, while Obsidian resolves same-file refs against the
        // whole file. Retry against a fresh full-file parse before declaring
        // the section missing; the guards above have already run, so any
        // re-expansion below stays bounded either way.
        let located = match located {
            Some(reduced) => Some(reduced),
            None if context.note_depth() == 1 => None,
            None => Self::parse_raw_note(context.current_file())
                .ok()
                .and_then(|(_frontmatter, full)| {
                    section.strip_prefix('^').map_or_else(
                        || reduce_to_section(&full, section),
                        |block_id| reduce_to_block(&full, block_id),
                    )
                }),
        };
        let mut events = match located {
            Some(reduced) => reduced,
            None => match self.missing_section_strategy {
                MissingSectionStrategy::EmbedFull => events.to_vec(),
                MissingSectionStrategy::Skip => {
                    self.warn(
                        Some(context.current_file()),
                        format!(
                            "Unable to find section '{section}' in note '{}'\n\tSource: '{}'\n",
                            context.current_file().display(),
                            context.current_file().display(),
                        ),
                    );
                    vec![]
                }
                MissingSectionStrategy::Fail => {
                    return Err(ExportError::SectionNotFound {
                        section: section.to_owned(),
                        path: context.current_file().clone(),
                    });
                }
            },
        };
        events = self.expand_references(&events, &child_context)?;
        for func in &self.embed_postprocessors {
            match func(&mut child_context, &mut events) {
                PostprocessorResult::StopHere => break,
                PostprocessorResult::StopAndSkipNote => {
                    events = vec![];
                }
                PostprocessorResult::Continue => (),
            }
        }
        Ok(events)
    }

    fn make_link_to_file<'c>(
        &self,
        reference: ObsidianNoteReference<'_>,
        context: &Context,
    ) -> MarkdownEvents<'c> {
        let target_file = reference.file.map_or_else(
            || Some(context.current_file()),
            |file| self.resolve_reference(file, context),
        );

        if target_file.is_none() {
            let current_file = context.current_file().to_string_lossy();
            self.warn(
                Some(context.current_file()),
                format!(
                    "Unable to find referenced note\n\tReference: '{}'\n\tSource: '{}'\n",
                    reference.file.unwrap_or_else(|| current_file.as_ref()),
                    context.current_file().display(),
                ),
            );
            return vec![
                Event::Start(Tag::Emphasis),
                Event::Text(CowStr::from(reference.display())),
                Event::End(TagEnd::Emphasis),
            ];
        }
        let target_file = target_file.unwrap();
        // We use root_file() rather than current_file() here to make sure links are always
        // relative to the outer-most note, which is the note which this content is inserted into
        // in case of embedded notes.
        let rel_link = diff_paths(
            target_file,
            context
                .root_file()
                .parent()
                .expect("obsidian content files should always have a parent"),
        )
        .expect("should be able to build relative path when target file is found in vault");

        // Plain-Markdown link destinations require forward slashes; on Windows the
        // platform-specific relative path from `diff_paths` would otherwise end up
        // with backslashes that most renderers can't resolve.
        let rel_link = rel_link.to_string_lossy().replace('\\', "/");
        let mut link = encode_link_destination(&rel_link);

        if let Some(section) = reference.section {
            link.push('#');
            link.push_str(&format_anchor(section));
        }

        let link_tag = Tag::Link {
            link_type: pulldown_cmark::LinkType::Inline,
            dest_url: CowStr::from(link),
            title: CowStr::from(""),
            id: CowStr::from(""),
        };

        vec![
            Event::Start(link_tag),
            Event::Text(CowStr::from(reference.display())),
            Event::End(TagEnd::Link),
        ]
    }
}

/// Get the full path for the given filename when it's contained in `vault_contents`, taking into
/// account:
///
/// 1. Standard Obsidian note references not including a .md extension.
/// 2. Case-insensitive matching
/// 3. Unicode normalization rules using normalization form C (<https://www.w3.org/TR/charmod-norm/#unicodeNormalization>)
///
/// When multiple files match (e.g. a bare-name reference while `Note.md` and `nested/Note.md`
/// both exist), the result is deterministic and independent of traversal order: the candidate
/// with the fewest path components wins, ties broken lexicographically.
///
/// This is a linear scan kept as the reference semantics for tests to compare
/// [`VaultIndex`] against; the export pipeline resolves references through the
/// prebuilt index. The two agree on Windows and on paths free of '\' characters;
/// on Unix a filename containing '\' can resolve differently between them.
#[cfg(test)]
fn lookup_filename_in_vault<'a>(
    filename: &str,
    vault_contents: &'a [PathBuf],
) -> Option<&'a PathBuf> {
    let filename = PathBuf::from(filename);
    let filename_normalized = filename.to_string_lossy().nfc().collect::<String>();
    let filename_normalized_lowered = filename_normalized.to_lowercase();

    vault_contents
        .iter()
        .filter(|path| {
            let path_normalized_str = path.to_string_lossy().nfc().collect::<String>();
            let path_normalized = PathBuf::from(&path_normalized_str);
            let path_normalized_lowered = PathBuf::from(&path_normalized_str.to_lowercase());

            // It would be convenient if we could just do `filename.set_extension("md")` at the
            // start of this funtion so we don't need multiple separate + ".md" match
            // cases here, however that would break with a reference of `[[Note.1]]`
            // linking to `[[Note.1.md]]`.

            path_normalized.ends_with(&filename_normalized)
                || path_normalized.ends_with(filename_normalized.clone() + ".md")
                || path_normalized_lowered.ends_with(&filename_normalized_lowered)
                || path_normalized_lowered.ends_with(filename_normalized_lowered.clone() + ".md")
        })
        .min_by_key(|path| {
            (
                path.components().count(),
                path.to_string_lossy().to_lowercase(),
            )
        })
}

/// Tie-break key deciding between multiple candidates for the same reference:
/// fewest path components first, then lexicographically smallest (case-insensitive).
fn lookup_tiebreak_key(path: &Path) -> (usize, String) {
    (
        path.components().count(),
        path.to_string_lossy().to_lowercase(),
    )
}

/// Prebuilt lookup index over vault contents, resolving reference strings (path
/// suffix, with or without `.md` extension, NFC-normalized and case-insensitive)
/// in constant time instead of a linear scan per reference.
///
/// For every vault file, every component-suffix of its normalized spelling is
/// inserted (e.g. `a/b/note.md` also answers `b/note` and `note`), with the
/// [`lookup_filename_in_vault`] tie-break rules applied deterministically at
/// build time. Lookups consult all four reference spellings and pick the best
/// candidate among them, so results are identical to the linear scan.
#[derive(Clone)]
struct VaultIndex {
    map: HashMap<String, PathBuf>,
}

impl VaultIndex {
    fn build(vault_contents: &[PathBuf]) -> Self {
        let mut map: HashMap<String, PathBuf> =
            HashMap::with_capacity(vault_contents.len().saturating_mul(4));

        for path in vault_contents {
            let normalized = path.to_string_lossy().nfc().collect::<String>();
            let normalized = normalized.replace('\\', "/");
            let lowered = normalized.to_lowercase();

            // Both the exact and lowercase spellings, with and without a `.md`
            // extension: a reference `[[Note.1]]` may point at `Note.1.md`, so the
            // extension-less variant of a `.md` file must also be indexed.
            let mut variants: Vec<String> = Vec::with_capacity(4);
            variants.push(normalized.clone());
            variants.push(lowered.clone());
            if let Some(stripped) = normalized.strip_suffix(".md") {
                variants.push(stripped.to_owned());
            }
            if let Some(stripped) = lowered.strip_suffix(".md") {
                variants.push(stripped.to_owned());
            }

            let new_key = lookup_tiebreak_key(path);
            for variant in variants {
                let components: Vec<&str> = variant.split('/').collect();
                for start in 0..components.len() {
                    let key = components
                        .iter()
                        .skip(start)
                        .copied()
                        .collect::<Vec<&str>>()
                        .join("/");
                    let replace = map
                        .get(key.as_str())
                        .is_none_or(|existing| new_key < lookup_tiebreak_key(existing));
                    if replace {
                        map.insert(key, path.clone());
                    }
                }
            }
        }
        Self { map }
    }

    fn lookup(&self, filename: &str) -> Option<&PathBuf> {
        let normalized = filename.nfc().collect::<String>().replace('\\', "/");
        let normalized_md = format!("{normalized}.md");
        let lowered = normalized.to_lowercase();
        let lowered_md = format!("{lowered}.md");
        let spellings = [normalized, normalized_md, lowered, lowered_md];
        let mut best: Option<&PathBuf> = None;
        for spelling in spellings {
            if let Some(candidate) = self.map.get(spelling.as_str()) {
                let better = best.is_none_or(|current| {
                    lookup_tiebreak_key(candidate) < lookup_tiebreak_key(current)
                });
                if better {
                    best = Some(candidate);
                }
            }
        }
        best
    }
}

/// Percent-encode only the characters whose presence would break a Markdown
/// inline link destination (or URL semantics): controls, spaces, parentheses,
/// `%`, `?`, `#`. Everything else — including non-ASCII characters such as
/// Chinese filenames — is kept verbatim, matching what Obsidian itself writes.
fn encode_link_destination(link: &str) -> String {
    link.chars()
        .map(|ch| {
            if ch.is_ascii_control() || matches!(ch, ' ' | '(' | ')' | '%' | '?' | '#') {
                // The branch condition guarantees the codepoint is below 0x80, so
                // two hex digits always suffice.
                format!("%{:02X}", u32::from(ch))
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Resolve `.` and `..` components in `path` without touching the filesystem.
///
/// When a `..` has no preceding normal component left to consume (e.g. it would
/// climb above the start of the path), it is kept as-is so that the resulting
/// path can never accidentally match a vault file.
fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else {
                    normalized.push("..");
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Generate a URL fragment for a section reference.
///
/// Delegates to the `github-slugger` crate so that generated anchors match
/// what common Markdown renderers such as GitHub and VS Code's preview
/// produce for a given heading: Unicode letters and digits (including CJK)
/// are kept as-is, underscores and hyphens are kept, each whitespace
/// character becomes a hyphen, and punctuation — ASCII or fullwidth — is
/// stripped outright without leaving a hyphen behind. Vectors for this
/// behavior were captured from live GitHub rendering (see
/// `test_format_anchor_matches_github_slugger`).
///
/// This function is stateless and therefore does not append GitHub's `-1`,
/// `-2`, … disambiguation suffixes for duplicate headings within a document;
/// a link reference always targets the first matching heading, whose slug
/// needs no suffix. Document-level suffixes only matter when *verifying*
/// hand-written fragments (the link checker slugs a target's headings in
/// document order with a stateful slugger). Leading and trailing whitespace
/// is trimmed first, matching VS Code's preview.
fn format_anchor(section: &str) -> String {
    github_slugger::slug(section.trim())
}

/// Render an error and its full source chain as a single string, so event consumers
/// see the root cause (e.g. "Failed to export 'x.md': Failed to decode YAML frontmatter
/// in 'x.md': ...") instead of only the outermost message.
fn error_chain_string(error: &ExportError) -> String {
    let mut chain = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(err) = source {
        chain.push_str(": ");
        chain.push_str(&err.to_string());
        source = err.source();
    }
    chain
}

/// Validate that the parent directory of a destination file exists.
///
/// A bare filename (e.g. `out.md`) has an empty parent component; its parent is really the
/// current directory, which is assumed to exist instead of reporting an empty path as missing.
fn validate_destination_parent(dest: &Path) -> Result<()> {
    let parent = match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !parent.exists() {
        return Err(ExportError::PathDoesNotExist {
            path: parent.to_path_buf(),
        });
    }
    Ok(())
}

fn render_mdevents_to_mdtext(markdown: &MarkdownEvents<'_>) -> String {
    let mut buffer = String::new();
    cmark_with_options(
        markdown.iter(),
        &mut buffer,
        pulldown_cmark_to_cmark::Options::default(),
    )
    .expect("formatting to string not expected to fail");
    buffer.push('\n');
    buffer
}

fn create_file(dest: &Path) -> Result<File> {
    let file = File::create(dest)
        .or_else(|err| {
            if err.kind() == ErrorKind::NotFound {
                let parent = dest.parent().expect("file should have a parent directory");
                fs::create_dir_all(parent)?;
                return File::create(dest);
            }
            Err(err)
        })
        .context(WriteSnafu { path: dest })?;
    Ok(file)
}

fn copy_mtime(src: &Path, dest: &Path) -> Result<()> {
    let metadata = fs::metadata(src).context(ModTimeReadSnafu { path: src })?;
    let modified_time = metadata
        .modified()
        .context(ModTimeReadSnafu { path: src })?;

    set_file_mtime(dest, modified_time.into()).context(ModTimeSetSnafu { path: dest })?;
    Ok(())
}

/// Copy a file from `src` to `dest`, creating parent directories if necessary.
///
/// The return signature looks a little convoluted but this is done to match
/// that of [`Exporter::parse_and_export_obsidian_note`].
fn copy_file(src: &Path, dest: &Path) -> Result<Option<PathBuf>> {
    fs::copy(src, dest)
        .or_else(|err| {
            if err.kind() == ErrorKind::NotFound {
                let parent = dest.parent().expect("file should have a parent directory");
                fs::create_dir_all(parent)?;
                return fs::copy(src, dest);
            }
            Err(err)
        })
        .context(WriteSnafu { path: dest })?;
    Ok(Some(dest.to_path_buf()))
}

fn is_markdown_file(file: &Path) -> bool {
    let no_ext = OsString::new();
    let ext = file.extension().unwrap_or(&no_ext).to_string_lossy();
    ext == "md"
}

/// The matching `TagEnd` for a block-level container `Tag`, or `None` for
/// non-containers. Used by [`reduce_to_section`] to keep returned event
/// slices balanced (start/end pairs intact) when cutting around blockquotes
/// and lists.
const fn block_container_end(tag: &Tag<'_>) -> Option<TagEnd> {
    match tag {
        Tag::BlockQuote(kind) => Some(TagEnd::BlockQuote(*kind)),
        Tag::List(start) => Some(TagEnd::List(start.is_some())),
        Tag::Item => Some(TagEnd::Item),
        Tag::FootnoteDefinition(_) => Some(TagEnd::FootnoteDefinition),
        _ => None,
    }
}

/// The pulldown-cmark parser flavor used for every note this crate parses.
/// The link checker reuses the same options so links are recognized with the
/// same extensions (tables, footnotes, GFM autolinks, …) as during export.
fn markdown_parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_GFM
}

/// A reference (`[[...]]` / `![[...]]`) extracted verbatim from a note by
/// [`Exporter::parse_raw_note_with_refs`]: the exact reference text plus the
/// byte offset where that text starts in the source.
struct RawNoteRef {
    embed: bool,
    text: String,
    start: usize,
}

/// Aggregate the inline text of a section query the same way heading
/// aggregation does inside [] (formatting events dropped,
/// Text/Code/InlineMath kept). Re-parsing the query as markdown makes a
/// reference like `![[note#*Target* Heading]]` match the heading it renders
/// to, while word-internal underscores (`my_note`) stay untouched.
///
/// The query is parsed as heading *inline* content (a `# ` prefix turns the
/// line into a single heading): block-level parsing would treat a leading
/// `N. `/`- `/`> ` as list or quote markup, consume the marker, and silently
/// break matching for every numbered/bulleted heading reference. The same
/// parser flavor as whole-note parsing keeps extension syntax (strikethrough,
/// inline math) aggregating identically on both sides of the comparison.
/// Known edge: a query ending in `" #"` is trimmed as an ATX closing
/// sequence on the query side only — a Setext heading whose text really ends
/// in ` #` would not match. Reaching it needs all of Setext spelling,
/// trailing ` #` and a verbatim reference at once, so it is accepted.
fn aggregate_inline_text(text: &str) -> String {
    let mut result = String::new();
    let query = format!("# {text}");
    for event in Parser::new_ext(&query, markdown_parser_options()) {
        match event {
            Event::Text(t) | Event::Code(t) | Event::InlineMath(t) => result.push_str(&t),
            Event::SoftBreak | Event::HardBreak => result.push(' '),
            _ => {}
        }
    }
    result
}

/// When `events[i]` opens a collapsed wikilink/embed — the canonical five
/// consecutive Text events emitted by `parse_raw_note` — return the text the
/// reference displays; otherwise `None`. The collapsed shape is unambiguous:
/// literal `[WIP]`-style brackets are replayed verbatim by the scanner and so
/// never contain a second opening bracket.
fn collapsed_ref_display(events: &[Event<'_>], i: usize, opener: &str) -> Option<String> {
    if opener != "[" && opener != "![" {
        return None;
    }
    let slice = i.checked_add(5).and_then(|end| events.get(i..end))?;
    // The collapsed shape is `[`/`![`, `[`, <literal>, `]`, `]`.
    let (
        Some(Event::Text(second)),
        Some(Event::Text(literal)),
        Some(Event::Text(close)),
        Some(Event::Text(end)),
    ) = (slice.get(1), slice.get(2), slice.get(3), slice.get(4))
    else {
        return None;
    };
    if second.as_ref() != "[" || close.as_ref() != "]" || end.as_ref() != "]" {
        return None;
    }
    Some(ObsidianNoteReference::from_str(literal.as_ref()).display())
}

/// Reduce a given `MarkdownEvents` to just those elements which are children of the given section
/// (heading name).
///
/// Returns `None` when no heading matches `section`, letting the caller decide how to handle
/// the missing section (see [`MissingSectionStrategy`]). Heading comparison aggregates all
/// inline content of the heading, including emphasis, inline code and math (so `## *Foo* Bar`
/// matches "Foo Bar"), and is case-insensitive as well as Unicode-normalized (NFC). A
/// wikilink/embed inside the heading aggregates by its display text (so `## [[mid]]` matches
/// "mid", the way the expanded link text did before the raw/expand split). When several
/// headings share the name, the first match wins and same-named headings nested deeper
/// are treated as regular content of that section.
fn reduce_to_section<'a>(events: &[Event<'a>], section: &str) -> Option<MarkdownEvents<'a>> {
    let section_normalized = aggregate_inline_text(section)
        .nfc()
        .collect::<String>()
        .to_lowercase();

    let mut filtered_events = Vec::with_capacity(events.len());
    // Block containers (blockquotes, lists, …) that are currently open. The
    // section cut may drop their Start or End events, so both return points
    // below re-balance the stream using this stack.
    let mut open_containers: Vec<Tag<'_>> = vec![];
    let mut target_section_encountered = false;
    let mut currently_in_target_section = false;
    let mut section_level = HeadingLevel::H1;
    let mut last_level = HeadingLevel::H1;
    let mut heading_start_idx = 0;
    let mut heading_text = String::new();
    let mut in_heading = false;

    let mut i = 0;
    while i < events.len() {
        let Some(event) = events.get(i) else {
            break;
        };
        if matches!(event, Event::Start(Tag::Heading { .. })) {
            heading_start_idx = filtered_events.len();
        }
        filtered_events.push(event.clone());
        // Aggregate the inline text that names the heading. A wikilink/embed
        // collapsed by parse_raw_note arrives as five consecutive Text events;
        // it aggregates by its display text so a section query matches what
        // the heading renders to. Literal single-layer brackets never form
        // that shape, so headings like "[WIP] Notes" keep aggregating
        // literally.
        if in_heading {
            match event {
                Event::Text(cowstr) => {
                    if let Some(display) = collapsed_ref_display(events, i, cowstr) {
                        heading_text.push_str(&display);
                        // The other four Text events join the stream verbatim
                        // and are skipped below; they are neither container
                        // nor heading events, so the loop bookkeeping is
                        // unaffected.
                        let (tail_start, tail_end) = (i.saturating_add(1), i.saturating_add(4));
                        for ev in events.get(tail_start..=tail_end).into_iter().flatten() {
                            filtered_events.push(ev.clone());
                        }
                        i = i.saturating_add(4);
                    } else {
                        heading_text.push_str(cowstr);
                    }
                }
                // Inline code and math inside a heading surface as these
                // events instead of Text; their literal text still counts
                // towards the heading name.
                Event::Code(cowstr) | Event::InlineMath(cowstr) => {
                    heading_text.push_str(cowstr);
                }
                Event::SoftBreak | Event::HardBreak => {
                    heading_text.push(' ');
                }
                _ => {}
            }
        }
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let level = *level;
                in_heading = true;
                heading_text.clear();
                last_level = level;
                if currently_in_target_section && level <= section_level {
                    currently_in_target_section = false;
                    filtered_events.pop();
                }
            }
            // Track block-level containers so the returned slice can be re-balanced.
            Event::Start(tag) => {
                if block_container_end(tag).is_some() {
                    open_containers.push(tag.clone());
                }
            }
            Event::End(
                TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item | TagEnd::FootnoteDefinition,
            ) => {
                open_containers.pop();
            }
            Event::End(TagEnd::Heading(_)) => {
                if in_heading {
                    in_heading = false;
                    // A same-named heading nested deeper than the target is simply part
                    // of the section content; only the first match starts the section.
                    if !currently_in_target_section
                        && heading_text.nfc().collect::<String>().to_lowercase()
                            == section_normalized
                    {
                        target_section_encountered = true;
                        currently_in_target_section = true;
                        section_level = last_level;

                        // Discard everything collected before the target heading; the heading
                        // itself (which may consist of multiple inline events) is kept. The
                        // prefix may contain the Start of containers the heading lives in:
                        // re-open them so the slice stays balanced.
                        let heading_events = filtered_events.split_off(heading_start_idx);
                        let mut balanced_events: MarkdownEvents<'a> = open_containers
                            .iter()
                            .map(|tag| Event::Start(tag.clone()))
                            .collect();
                        balanced_events.extend(heading_events);
                        filtered_events = balanced_events;
                    }
                }
            }
            _ => {}
        }
        if target_section_encountered && !currently_in_target_section {
            // The terminating heading was inside one or more containers whose End
            // events never arrive: close them so the slice stays balanced.
            for tag in open_containers.iter().rev() {
                filtered_events.push(Event::End(
                    block_container_end(tag).expect("tracked tags are block containers"),
                ));
            }
            return Some(filtered_events);
        }
        i = i.saturating_add(1);
    }
    target_section_encountered.then_some(filtered_events)
}

fn is_valid_block_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The trailing block-id marker of an Obsidian block reference: content
/// ending in ` ^block-id` marks the block it ends.
fn trailing_block_id(text: &str) -> Option<&str> {
    let (_, id) = text.rsplit_once(" ^")?;
    is_valid_block_id(id).then_some(id)
}

/// Strip the ` ^block-id` marker: embedded blocks don't show their id.
fn strip_trailing_block_id_marker(text: &str) -> CowStr<'static> {
    let (stripped, _) = text
        .rsplit_once(" ^")
        .expect("candidate text matched a trailing block id");
    CowStr::from(stripped.to_owned())
}

/// A standalone id line (`^block-id` alone on its own paragraph) marking the
/// block above it.
fn standalone_block_id(text: &str) -> Option<&str> {
    let id = text.strip_prefix('^')?;
    is_valid_block_id(id).then_some(id)
}

/// Whether a `Tag` opens a block-level element (as opposed to inline
/// formatting).
const fn is_block_tag(tag: &Tag<'_>) -> bool {
    !matches!(
        tag,
        Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::Link { .. }
            | Tag::Image { .. }
    )
}

/// Whether a `TagEnd` closes a block-level element; counterpart of
/// [`is_block_tag`].
const fn is_block_end(tag_end: TagEnd) -> bool {
    !matches!(
        tag_end,
        TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::Link
            | TagEnd::Image
    )
}

/// Reduce a `MarkdownEvents` stream to the single block marked by an Obsidian
/// block id (`![[note#^block-id]]`).
///
/// Mirrors Obsidian's block semantics: an id at the end of a paragraph marks
/// that paragraph (or, inside a list, the list item it belongs to; at the end
/// of a quote block, the whole quote); an id alone on a line of its own marks
/// the block directly above it. The id marker itself is stripped from the
/// returned events. Returns `None` when no block carries the id, letting the
/// caller fall back to [`MissingSectionStrategy`].
#[allow(clippy::too_many_lines)]
fn reduce_to_block<'a>(events: &[Event<'a>], block_id: &str) -> Option<MarkdownEvents<'a>> {
    use std::collections::HashMap;

    // Pass 1: pair up block-level Start/End events, record top-level block
    // ranges and collect id candidates.
    let mut block_stack: Vec<usize> = vec![]; // indexes of Start events
    let mut block_end: HashMap<usize, usize> = HashMap::new();
    let mut toplevel_blocks: Vec<(usize, usize)> = vec![]; // (Start idx, End idx)
    let mut trailing: Vec<(usize, String, Vec<usize>)> = vec![]; // (Text idx, id, stack)
    let mut standalone: Vec<(usize, String)> = vec![]; // (Text idx, id) of `^id`-only paragraphs

    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::Start(tag) if is_block_tag(tag) => {
                block_stack.push(idx);
            }
            Event::End(tag_end) if is_block_end(*tag_end) => {
                if let Some(start_idx) = block_stack.pop() {
                    block_end.insert(start_idx, idx);
                    if block_stack.is_empty() {
                        toplevel_blocks.push((start_idx, idx));
                    }
                }
            }
            Event::Text(text) => {
                let in_code_block = block_stack.iter().any(|&start| {
                    matches!(events.get(start), Some(Event::Start(Tag::CodeBlock(_))))
                });
                if !in_code_block {
                    // Text inside a code block is literal content, never a
                    // block marker.
                    match trailing_block_id(text) {
                        Some(id) => trailing.push((idx, id.to_owned(), block_stack.clone())),
                        None => {
                            if block_stack.len() == 1 {
                                if let Some(id) = standalone_block_id(text) {
                                    // Standalone ids only mark blocks when
                                    // alone in a top-level paragraph.
                                    standalone.push((idx, id.to_owned()));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Pass 2: resolve the requested id. A trailing id wins over a standalone
    // id with the same text.
    if let Some((text_idx, _, snapshot)) = trailing.iter().find(|(_, id, _)| id == block_id) {
        let text_idx = *text_idx;
        // Prefer the nearest enclosing list item (ids on a list bullet resolve
        // to that item); then the innermost enclosing quote block (an id in a
        // nested quote marks the innermost quote, not the whole outer one);
        // otherwise the whole enclosing top-level block (a paragraph).
        let item_pos = snapshot
            .iter()
            .rposition(|&start| matches!(events.get(start), Some(Event::Start(Tag::Item))));
        if let Some(item_pos) = item_pos {
            let item_start = snapshot.get(item_pos).copied()?;
            let end_idx = block_end.get(&item_start).copied()?;
            let slice = events.get(item_start..=end_idx)?;
            // Strip the ` ^block-id` marker from the item's trailing text.
            let stripped: MarkdownEvents<'a> = (item_start..)
                .zip(slice.iter())
                .map(|(idx, event)| {
                    if idx == text_idx {
                        match event {
                            Event::Text(text) => Event::Text(strip_trailing_block_id_marker(text)),
                            other => other.clone(),
                        }
                    } else {
                        event.clone()
                    }
                })
                .collect();
            // Re-wrap the item in its list: a bare Item renders without its
            // bullet/number. The parent list's tag is right below the item on
            // the block stack; fall back to an unordered list defensively.
            let list_start = item_pos
                .checked_sub(1)
                .and_then(|pos| snapshot.get(pos).copied());
            let (list_start_event, list_end_event) =
                match list_start.and_then(|start| events.get(start)) {
                    Some(Event::Start(tag @ Tag::List(_))) => {
                        (tag.clone(), TagEnd::List(matches!(tag, Tag::List(Some(_)))))
                    }
                    _ => (Tag::List(None), TagEnd::List(false)),
                };
            let mut wrapped: MarkdownEvents<'a> = vec![];
            // An item inside a quote keeps its quote context.
            let outer_snapshot = snapshot.get(..item_pos)?;
            let quote_start =
                outer_snapshot.iter().copied().rev().find(|&start| {
                    matches!(events.get(start), Some(Event::Start(Tag::BlockQuote(_))))
                });
            let quote_end = quote_start.and_then(|start| {
                let tag = match events.get(start) {
                    Some(Event::Start(tag @ Tag::BlockQuote(_))) => tag.clone(),
                    _ => return None,
                };
                let kind = match &tag {
                    Tag::BlockQuote(kind) => *kind,
                    _ => None,
                };
                wrapped.push(Event::Start(tag));
                Some(Event::End(TagEnd::BlockQuote(kind)))
            });
            wrapped.push(Event::Start(list_start_event));
            wrapped.extend(stripped);
            wrapped.push(Event::End(list_end_event));
            if let Some(quote_end) = quote_end {
                wrapped.push(quote_end);
            }
            return Some(wrapped);
        }
        // Innermost enclosing quote block: a quote-ending id marks that quote.
        let quote_start = snapshot
            .iter()
            .copied()
            .rev()
            .find(|&start| matches!(events.get(start), Some(Event::Start(Tag::BlockQuote(_)))));
        let start_idx = match quote_start {
            Some(start_idx) => start_idx,
            None => toplevel_blocks
                .iter()
                .find(|(s, e)| *s <= text_idx && text_idx <= *e)
                .map(|(s, _)| *s)?,
        };
        let end_idx = block_end.get(&start_idx).copied()?;
        let slice = events.get(start_idx..=end_idx)?;
        return Some(
            (start_idx..)
                .zip(slice.iter())
                .map(|(idx, event)| {
                    if idx == text_idx {
                        match event {
                            Event::Text(text) => Event::Text(strip_trailing_block_id_marker(text)),
                            other => other.clone(),
                        }
                    } else {
                        event.clone()
                    }
                })
                .collect(),
        );
    }

    if let Some((text_idx, _)) = standalone.iter().find(|(_, id)| id == block_id) {
        // The standalone id line marks the closest top-level block *entirely
        // above* it — not the id paragraph itself.
        if let Some((start_idx, end_idx)) = toplevel_blocks
            .iter()
            .filter(|(_, e)| e < text_idx)
            .max_by_key(|(s, _)| *s)
        {
            return events.get(*start_idx..=*end_idx).map(<[Event<'_>]>::to_vec);
        }
    }
    None
}

fn event_to_owned<'a>(event: Event<'_>) -> Event<'a> {
    match event {
        Event::Start(tag) => Event::Start(tag_to_owned(tag)),
        Event::End(tag) => Event::End(tag),
        Event::Text(cowstr) => Event::Text(CowStr::from(cowstr.into_string())),
        Event::Code(cowstr) => Event::Code(CowStr::from(cowstr.into_string())),
        Event::Html(cowstr) => Event::Html(CowStr::from(cowstr.into_string())),
        Event::InlineHtml(cowstr) => Event::InlineHtml(CowStr::from(cowstr.into_string())),
        Event::FootnoteReference(cowstr) => {
            Event::FootnoteReference(CowStr::from(cowstr.into_string()))
        }
        Event::SoftBreak => Event::SoftBreak,
        Event::HardBreak => Event::HardBreak,
        Event::Rule => Event::Rule,
        Event::TaskListMarker(checked) => Event::TaskListMarker(checked),
        Event::InlineMath(cowstr) => Event::InlineMath(CowStr::from(cowstr.into_string())),
        Event::DisplayMath(cowstr) => Event::DisplayMath(CowStr::from(cowstr.into_string())),
    }
}

fn tag_to_owned<'a>(tag: Tag<'_>) -> Tag<'a> {
    match tag {
        Tag::Paragraph => Tag::Paragraph,
        Tag::Heading {
            level: heading_level,
            id,
            classes,
            attrs,
        } => Tag::Heading {
            level: heading_level,
            id: id.map(|cowstr| CowStr::from(cowstr.into_string())),
            classes: classes
                .into_iter()
                .map(|cowstr| CowStr::from(cowstr.into_string()))
                .collect(),
            attrs: attrs
                .into_iter()
                .map(|(attr, value)| {
                    (
                        CowStr::from(attr.into_string()),
                        value.map(|cowstr| CowStr::from(cowstr.into_string())),
                    )
                })
                .collect(),
        },
        Tag::BlockQuote(blockquote_kind) => Tag::BlockQuote(blockquote_kind),
        Tag::CodeBlock(codeblock_kind) => Tag::CodeBlock(codeblock_kind_to_owned(codeblock_kind)),
        Tag::List(optional) => Tag::List(optional),
        Tag::Item => Tag::Item,
        Tag::FootnoteDefinition(cowstr) => {
            Tag::FootnoteDefinition(CowStr::from(cowstr.into_string()))
        }
        Tag::Table(alignment_vector) => Tag::Table(alignment_vector),
        Tag::TableHead => Tag::TableHead,
        Tag::TableRow => Tag::TableRow,
        Tag::TableCell => Tag::TableCell,
        Tag::Emphasis => Tag::Emphasis,
        Tag::Strong => Tag::Strong,
        Tag::Strikethrough => Tag::Strikethrough,
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => Tag::Link {
            link_type,
            dest_url: CowStr::from(dest_url.into_string()),
            title: CowStr::from(title.into_string()),
            id: CowStr::from(id.into_string()),
        },
        Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        } => Tag::Image {
            link_type,
            dest_url: CowStr::from(dest_url.into_string()),
            title: CowStr::from(title.into_string()),
            id: CowStr::from(id.into_string()),
        },
        Tag::HtmlBlock => Tag::HtmlBlock,
        Tag::MetadataBlock(metadata_block_kind) => Tag::MetadataBlock(metadata_block_kind),
        Tag::DefinitionList => Tag::DefinitionList,
        Tag::DefinitionListTitle => Tag::DefinitionListTitle,
        Tag::DefinitionListDefinition => Tag::DefinitionListDefinition,
        Tag::Subscript => Tag::Subscript,
        Tag::Superscript => Tag::Superscript,
    }
}

fn codeblock_kind_to_owned<'a>(codeblock_kind: CodeBlockKind<'_>) -> CodeBlockKind<'a> {
    match codeblock_kind {
        CodeBlockKind::Indented => CodeBlockKind::Indented,
        CodeBlockKind::Fenced(cowstr) => CodeBlockKind::Fenced(CowStr::from(cowstr.into_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    static VAULT: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
        vec![
            PathBuf::from("NoteA.md"),
            PathBuf::from("Document.pdf"),
            PathBuf::from("Note.1.md"),
            PathBuf::from("nested/NoteA.md"),
            PathBuf::from("Note\u{E4}.md"), // Noteä.md, see also encodings() below
        ]
    });

    #[test]
    #[allow(clippy::unicode_not_nfc)]
    fn encodings() {
        // Standard "Latin Small Letter A with Diaeresis" (U+00E4)
        // Encoded in UTF-8 as two bytes: 0xC3 0xA4
        assert_eq!(String::from_utf8(vec![0xC3, 0xA4]).unwrap(), "ä");
        assert_eq!("\u{E4}", "ä");

        // Basic (ASCII) lowercase a followed by Unicode Character “◌̈” (U+0308)
        // Renders the same visual appearance but is encoded in UTF-8 as three bytes:
        // 0x61 0xCC 0x88
        assert_eq!(String::from_utf8(vec![0x61, 0xCC, 0x88]).unwrap(), "ä");
        assert_eq!("a\u{308}", "ä");
        assert_eq!("\u{61}\u{308}", "ä");

        // For more examples and a better explanation of this concept, see
        // https://www.w3.org/TR/charmod-norm/#aringExample
    }

    #[test]
    fn test_format_anchor_preserves_unicode() {
        // CJK headings must survive as-is; the previous slug-based implementation
        // transliterated these into pinyin, producing anchors no renderer matches.
        assert_eq!(format_anchor("中文标题"), "中文标题");
        assert_eq!(format_anchor("混合 Heading 标题"), "混合-heading-标题");
    }

    #[test]
    fn test_format_anchor_english_compatibility() {
        // Anchors for plain English headings stay identical to the old slug behavior.
        assert_eq!(format_anchor("Heading One"), "heading-one");
        assert_eq!(format_anchor("with heading"), "with-heading");
        assert_eq!(format_anchor("dda637"), "dda637");
    }

    #[test]
    fn test_format_anchor_keeps_underscores() {
        // GitHub's slugger keeps underscores (they're word characters); links like
        // [note#Foo_Bar] must produce anchors that resolve on such renderers.
        assert_eq!(format_anchor("Foo_Bar"), "foo_bar");
        assert_eq!(format_anchor("snake_case_heading"), "snake_case_heading");
    }

    #[test]
    fn test_format_anchor_strips_punctuation() {
        // Punctuation is removed without leaving a hyphen behind (GitHub's
        // slugger deletes the character outright; it does not replace it).
        assert_eq!(format_anchor("C++ and Rust!"), "c-and-rust");
        // Each space becomes one hyphen; runs are not collapsed, and
        // surrounding whitespace is trimmed first.
        assert_eq!(format_anchor("  spaced   out  "), "spaced---out");
        // Hyphens are ordinary characters to GitHub: neither collapsed nor
        // trimmed from the ends.
        assert_eq!(format_anchor("-dashed-"), "-dashed-");
    }

    #[test]
    fn test_format_anchor_matches_github_slugger() {
        // Vectors captured from live GitHub rendering in 2026-08: GitHub and
        // VS Code strip punctuation — including fullwidth CJK forms such as
        // '：' (U+FF1A) and '，' (U+FF0C) — without leaving a hyphen behind.
        // Keeping such punctuation in a generated anchor produces links that
        // resolve on neither renderer.
        assert_eq!(
            format_anchor("总纲：三份形态，两个断口"),
            "总纲三份形态两个断口"
        );
        assert_eq!(
            format_anchor("断口-a：入库前，输入已非原话"),
            "断口-a入库前输入已非原话"
        );
        assert_eq!(
            format_anchor("对照：半角逗号, 句号. 与空格"),
            "对照半角逗号-句号-与空格"
        );
        // Numbered headings (e.g. produced by the "Number Headings" plugin):
        // periods vanish, so '1.1.1 C' must not become '1-1-1-c'.
        assert_eq!(format_anchor("1.1.1 C"), "111-c");
        assert_eq!(format_anchor("1.2.3.4"), "1234");
        assert_eq!(format_anchor("this--or-that"), "this--or-that");
    }

    #[test]
    fn test_aggregate_inline_text_keeps_block_markers_literal() {
        // A section query is re-parsed as Markdown to aggregate the text it
        // renders to — but as heading *inline* content. A leading `N. ` or
        // `- ` in a query names a numbered/bulleted heading and is heading
        // text, not a list marker: block-level parsing would consume the
        // marker and silently break matching for every numbered heading.
        assert_eq!(
            aggregate_inline_text("5. Numbered Section"),
            "5. Numbered Section"
        );
        assert_eq!(
            aggregate_inline_text("- dashed heading"),
            "- dashed heading"
        );
        assert_eq!(aggregate_inline_text("1) paren"), "1) paren");
        assert_eq!(aggregate_inline_text("+ plus heading"), "+ plus heading");
        assert_eq!(
            aggregate_inline_text("> quoted heading"),
            "> quoted heading"
        );
        // Fullwidth ordinals are not ASCII digits, so CommonMark's list rules
        // never consumed them anyway; asserted for symmetry with the halfwidth
        // case as parser behavior evolves.
        assert_eq!(aggregate_inline_text("５. 全角序号"), "５. 全角序号");
        // Inline formatting still aggregates to its rendered text, and
        // word-internal underscores stay literal. Extension syntax uses the
        // same parser flavor as whole-note parsing.
        assert_eq!(aggregate_inline_text("*Target* Heading"), "Target Heading");
        assert_eq!(aggregate_inline_text("my_note"), "my_note");
        assert_eq!(aggregate_inline_text("~~gone~~ tail"), "gone tail");
        assert_eq!(aggregate_inline_text("$x$ tail"), "x tail");
    }

    #[test]
    fn test_link_destination_encoding() {
        // A '#' in a path segment would otherwise be read as a fragment separator
        // once the segment lands in a generated Markdown link.
        assert_eq!(encode_link_destination("a#b.md"), "a%23b.md");
        // Spaces and parentheses would terminate or unbalance an inline link
        // destination, so they must be escaped.
        assert_eq!(
            encode_link_destination("a b(c).md?q"),
            "a%20b%28c%29.md%3Fq"
        );
        // Non-ASCII characters such as Chinese filenames stay verbatim, matching
        // what Obsidian itself writes; renderers handle Unicode paths fine.
        assert_eq!(encode_link_destination("笔记/图.svg"), "笔记/图.svg");
        assert_eq!(
            encode_link_destination("Nöte with 'quotes'.md"),
            "Nöte%20with%20'quotes'.md"
        );
    }

    #[test]
    fn test_error_chain_string_joins_full_chain() {
        let error = ExportError::FileExportError {
            path: PathBuf::from("note.md"),
            source: Box::new(ExportError::PathDoesNotExist {
                path: PathBuf::from("missing.md"),
            }),
        };
        let chain = error_chain_string(&error);
        assert!(
            chain.contains("Failed to export"),
            "outer context, got: {:?}",
            chain
        );
        assert!(
            chain.contains("No such file or directory"),
            "root cause, got: {:?}",
            chain
        );
        assert!(
            chain.contains(": "),
            "links joined with colons, got: {:?}",
            chain
        );
    }

    #[test]
    fn test_destination_parent_validation() {
        // A bare filename treats the current directory as its parent instead of
        // reporting an empty path as missing.
        validate_destination_parent(Path::new("out.md")).unwrap();
        assert!(validate_destination_parent(Path::new("no-such-dir/out.md")).is_err());
        match validate_destination_parent(Path::new("no-such-dir/out.md")) {
            Err(ExportError::PathDoesNotExist { path }) => {
                assert!(!path.as_os_str().is_empty());
            }
            _ => panic!("expected PathDoesNotExist with a non-empty path"),
        }
    }

    #[test]
    fn test_lookup_same_filename_is_deterministic() {
        // A bare-name reference hitting multiple candidates must resolve to the same
        // file regardless of the order files were discovered in: shortest path first,
        // then lexicographically smallest.
        let vault_one = vec![PathBuf::from("NoteA.md"), PathBuf::from("nested/NoteA.md")];
        let vault_two = vec![PathBuf::from("nested/NoteA.md"), PathBuf::from("NoteA.md")];
        let expected = PathBuf::from("NoteA.md");
        assert_eq!(
            lookup_filename_in_vault("NoteA", &vault_one),
            Some(&expected)
        );
        assert_eq!(
            lookup_filename_in_vault("NoteA", &vault_two),
            Some(&expected)
        );

        let vault_lex = vec![PathBuf::from("b/NoteA.md"), PathBuf::from("a/NoteA.md")];
        let expected_lex = PathBuf::from("a/NoteA.md");
        assert_eq!(
            lookup_filename_in_vault("NoteA", &vault_lex),
            Some(&expected_lex)
        );

        // A path-qualified reference keeps pointing at the nested file, not the
        // shorter bare-name candidate.
        let expected_nested = PathBuf::from("nested/NoteA.md");
        assert_eq!(
            lookup_filename_in_vault("nested/NoteA", &vault_two),
            Some(&expected_nested)
        );

        // The prebuilt index must agree with the linear scan on all of the above,
        // including the same-depth lexicographic tie-break.
        assert_eq!(
            VaultIndex::build(&vault_one).lookup("NoteA"),
            Some(&expected)
        );
        assert_eq!(
            VaultIndex::build(&vault_two).lookup("NoteA"),
            Some(&expected)
        );
        assert_eq!(
            VaultIndex::build(&vault_lex).lookup("NoteA"),
            Some(&expected_lex)
        );
        assert_eq!(
            VaultIndex::build(&vault_two).lookup("nested/NoteA"),
            Some(&expected_nested)
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_lookup_backslash_separator_agrees_between_paths() {
        // On Windows both '\' and '/' are component separators; the linear scan (via
        // Path component matching) and the index (via explicit replacement) must
        // resolve such references identically.
        let vault = vec![PathBuf::from("dir/file.md")];
        let index = VaultIndex::build(&vault);
        assert_eq!(
            lookup_filename_in_vault("dir\\file", &vault),
            index.lookup("dir\\file")
        );
    }

    fn parse_events(text: &str) -> MarkdownEvents<'static> {
        Parser::new(text).map(event_to_owned).collect()
    }

    #[test]
    fn test_reduce_to_section_found() {
        let events =
            parse_events("# First\n\nfirst.\n\n## Target\n\ntarget content.\n\n## After\n\nafter.");
        let reduced = reduce_to_section(&events, "Target").expect("section should be found");
        let rendered = render_mdevents_to_mdtext(&reduced);
        assert!(rendered.contains("## Target"), "heading kept: {}", rendered);
        assert!(
            rendered.contains("target content"),
            "content kept: {}",
            rendered
        );
        assert!(
            !rendered.contains("first"),
            "prior section dropped: {}",
            rendered
        );
        assert!(
            !rendered.contains("after"),
            "following section dropped: {}",
            rendered
        );
    }

    #[test]
    fn test_reduce_to_section_missing_returns_none() {
        let events = parse_events("# First\n\nfirst.");
        assert!(reduce_to_section(&events, "Nope").is_none());
    }

    #[test]
    fn test_reduce_to_section_matches_formatted_heading() {
        // A heading with inline formatting arrives as several text events; matching must
        // aggregate them all instead of comparing only the first fragment.
        let events = parse_events("# First\n\nfirst.\n\n## *Target* Heading\n\ncontent.");
        let reduced =
            reduce_to_section(&events, "Target Heading").expect("formatted heading should match");
        let rendered = render_mdevents_to_mdtext(&reduced);
        assert!(
            rendered.contains("content."),
            "section content kept: {}",
            rendered
        );
    }

    #[test]
    fn test_reduce_to_section_heading_with_inline_code() {
        // Headings containing inline code (or math) surface as Code/InlineMath events
        // rather than Text; aggregation must include their text, or the section would
        // wrongly look missing.
        let events = parse_events("# First\n\nfirst.\n\n## `code` heading\n\ncontent.");
        let reduced =
            reduce_to_section(&events, "code heading").expect("inline-code heading should match");
        let rendered = render_mdevents_to_mdtext(&reduced);
        assert!(
            rendered.contains("content."),
            "section content kept: {}",
            rendered
        );
    }

    #[test]
    fn test_reduce_to_section_nested_duplicate_heading_keeps_first_section() {
        // A same-named heading nested deeper than the target must not restart the
        // section: the embed runs from the first match to the end of that section.
        let events = parse_events("## Target\n\nouter.\n\n### Target\n\ninner.\n");
        let reduced = reduce_to_section(&events, "Target").expect("section should be found");
        let rendered = render_mdevents_to_mdtext(&reduced);
        assert!(
            rendered.contains("outer."),
            "outer content kept: {}",
            rendered
        );
        assert!(
            rendered.contains("### Target"),
            "nested same-named heading kept: {}",
            rendered
        );
    }

    #[test]
    fn test_reduce_to_section_matching_is_case_insensitive_and_nfc() {
        let events = parse_events("# First\n\nfirst.\n## Café\n\ncontent.");
        // NFD ("e" + combining diaeresis) should still match the NFC heading above.
        let section_nfd = "Cafe\u{301}";
        let reduced = reduce_to_section(&events, section_nfd).expect("NFC/NFD should match");
        assert!(render_mdevents_to_mdtext(&reduced).contains("content."));
        assert!(
            reduce_to_section(&events, "café").is_some(),
            "case-insensitive match"
        );
    }

    fn assert_block_containers_balanced(events: &[Event<'_>], context: &str) {
        let mut stack = vec![];
        for event in events {
            match event {
                Event::Start(tag) => {
                    if let Some(end) = block_container_end(tag) {
                        stack.push(end);
                    }
                }
                Event::End(
                    end @ (TagEnd::BlockQuote(_)
                    | TagEnd::List(_)
                    | TagEnd::Item
                    | TagEnd::FootnoteDefinition),
                ) => {
                    let top = stack
                        .pop()
                        .unwrap_or_else(|| panic!("{}: End {:?} without a Start", context, end));
                    assert_eq!(top, *end, "{}: mismatched container pair", context);
                }
                _ => {}
            }
        }
        assert!(
            stack.is_empty(),
            "{}: unclosed containers: {:?}",
            context,
            stack
        );
    }

    #[test]
    fn test_reduce_to_section_strips_emphasis_markers_from_query() {
        // A reference like ![[note#*Target* Heading]] carries the section name
        // with its literal markers; matching must strip them just like it
        // strips them from the heading's inline events.
        let events = parse_events(
            "# First

first.

## *Target* Heading

content.",
        );
        let reduced = reduce_to_section(&events, "*Target* Heading")
            .expect("query with markers should match");
        assert!(render_mdevents_to_mdtext(&reduced).contains("content."));
        let dunder_events = parse_events(
            "## __dunder__

content.",
        );
        assert!(
            reduce_to_section(&dunder_events, "__dunder__").is_some(),
            "underscore spellings match their own headings"
        );
    }

    #[test]
    fn test_reduce_to_section_target_heading_inside_blockquote() {
        // The target heading lives inside a blockquote: dropping the pre-heading
        // prefix must not discard the blockquote's Start event, or the stray End
        // unbalances the stream and pulls surrounding output into the quote.
        let events = parse_events("# Intro\n\nintro.\n\n> ## Target\n> quoted.");
        let reduced = reduce_to_section(&events, "Target").expect("section should be found");
        assert_block_containers_balanced(&reduced, "target heading in blockquote");
        let rendered = render_mdevents_to_mdtext(&reduced);
        assert!(
            rendered.contains("> ## Target"),
            "heading keeps its quote context: {}",
            rendered
        );
        assert!(
            rendered.contains("quoted."),
            "quote content kept: {}",
            rendered
        );
    }

    #[test]
    fn test_reduce_to_section_terminating_heading_inside_blockquote() {
        // A terminating heading inside a blockquote must not leave the blockquote
        // Start unclosed in the returned events.
        let events = parse_events("## Target\n\ntext.\n\n> ## Other\n> other text.");
        let reduced = reduce_to_section(&events, "Target").expect("section should be found");
        assert_block_containers_balanced(&reduced, "terminating heading in blockquote");
        let rendered = render_mdevents_to_mdtext(&reduced);
        assert!(
            rendered.contains("text."),
            "section content kept: {}",
            rendered
        );
        assert!(
            !rendered.contains("other"),
            "terminating blockquote heading excluded: {}",
            rendered
        );
    }

    /// The canonical five Text events `parse_raw_note` collapses a
    /// wikilink/embed into: `[`/`![`, `[`, <reference text>, `]`, `]`.
    fn collapsed_ref_events(opener: &str, literal: &str) -> Vec<Event<'static>> {
        [
            opener.to_owned(),
            "[".to_owned(),
            literal.to_owned(),
            "]".to_owned(),
            "]".to_owned(),
        ]
        .iter()
        .map(|text| Event::Text(CowStr::from(text.clone())))
        .collect()
    }

    fn h2_heading() -> Event<'static> {
        Event::Start(Tag::Heading {
            level: HeadingLevel::H2,
            id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
        })
    }

    fn section_note_with_heading_inline(inline: Vec<Event<'static>>) -> MarkdownEvents<'static> {
        let mut events = vec![h2_heading()];
        events.extend(inline);
        events.push(Event::End(TagEnd::Heading(HeadingLevel::H2)));
        events.push(Event::Start(Tag::Paragraph));
        events.push(Event::Text(CowStr::from("content.")));
        events.push(Event::End(TagEnd::Paragraph));
        events
    }

    #[test]
    fn test_reduce_to_section_heading_with_collapsed_wikilink() {
        // A wikilink inside a heading arrives from parse_raw_note as the five
        // collapsed Text events; aggregation must use the display text ("mid")
        // instead of the literal brackets, so `![[note#mid]]` resolves such a
        // heading again (as it did before the raw/expand split).
        let events = section_note_with_heading_inline(collapsed_ref_events("[", "mid"));
        let reduced = reduce_to_section(&events, "mid").expect("display text should match");
        // The returned stream keeps the collapsed events verbatim; expanding
        // them into a real link is expand_references' job.
        assert!(
            reduced.contains(&Event::Text(CowStr::from("["))),
            "collapsed events kept verbatim: {:?}",
            reduced
        );
        assert!(
            render_mdevents_to_mdtext(&reduced).contains("content."),
            "section content kept"
        );
    }

    #[test]
    fn test_reduce_to_section_collapsed_wikilink_uses_display_name() {
        // `mid|alias` displays as the label; the file part alone must not match.
        let events = section_note_with_heading_inline(collapsed_ref_events("[", "mid|alias"));
        assert!(reduce_to_section(&events, "alias").is_some());
        assert!(reduce_to_section(&events, "mid").is_none());
    }

    #[test]
    fn test_reduce_to_section_collapsed_embed_heading() {
        // Embeds collapse with the "![" opener; `![[note#sec]]` in a heading
        // aggregates as its display text "note > sec".
        let events = section_note_with_heading_inline(collapsed_ref_events("![", "note#sec"));
        assert!(reduce_to_section(&events, "note > sec").is_some());
        assert!(reduce_to_section(&events, "note#sec").is_none());
    }

    #[test]
    fn test_reduce_to_section_literal_brackets_in_heading_stay_literal() {
        // Single-layer brackets (e.g. "[WIP]") are plain text the scanner
        // replayed verbatim; they must keep aggregating literally so the query
        // "[WIP] Notes" matches while the unwrapped "WIP Notes" does not.
        let inline = vec![
            Event::Text(CowStr::from("[")),
            Event::Text(CowStr::from("WIP")),
            Event::Text(CowStr::from("]")),
            Event::Text(CowStr::from(" Notes")),
        ];
        let events = section_note_with_heading_inline(inline);
        assert!(reduce_to_section(&events, "[WIP] Notes").is_some());
        assert!(reduce_to_section(&events, "WIP Notes").is_none());
    }

    #[rstest]
    // Exact match
    #[case("NoteA.md", "NoteA.md")]
    #[case("NoteA", "NoteA.md")]
    // Same note in subdir, exact match should find it
    #[case("nested/NoteA.md", "nested/NoteA.md")]
    #[case("nested/NoteA", "nested/NoteA.md")]
    // Different extensions
    #[case("Document.pdf", "Document.pdf")]
    #[case("Note.1", "Note.1.md")]
    #[case("Note.1.md", "Note.1.md")]
    // Case-insensitive matches
    #[case("notea.md", "NoteA.md")]
    #[case("notea", "NoteA.md")]
    #[case("NESTED/notea.md", "nested/NoteA.md")]
    #[case("NESTED/notea", "nested/NoteA.md")]
    // "Latin Small Letter A with Diaeresis" (U+00E4)
    #[case("Note\u{E4}.md", "Note\u{E4}.md")]
    #[case("Note\u{E4}", "Note\u{E4}.md")]
    // Basic (ASCII) lowercase a followed by Unicode Character “◌̈” (U+0308)
    // The UTF-8 encoding is different but it renders the same visual appearance as the case above,
    // so we expect it to find the same file.
    #[case("Note\u{61}\u{308}.md", "Note\u{E4}.md")]
    #[case("Note\u{61}\u{308}", "Note\u{E4}.md")]
    // We should expect this to work with lowercasing as well, so NoteÄ should find Noteä
    // NoteÄ where Ä = Single Ä (U+00C4)
    #[case("Note\u{C4}.md", "Note\u{E4}.md")]
    #[case("Note\u{C4}", "Note\u{E4}.md")]
    // NoteÄ where Ä = decomposed to A (U+0041) + ◌̈ (U+0308)
    #[case("Note\u{41}\u{308}.md", "Note\u{E4}.md")]
    #[case("Note\u{41}\u{308}", "Note\u{E4}.md")]
    fn test_lookup_filename_in_vault(#[case] input: &str, #[case] expected: &str) {
        let empty_path = PathBuf::new();
        let result = lookup_filename_in_vault(input, &VAULT);
        println!("Test input: {input:?}");
        println!("Expecting: {expected:?}");
        println!("Got: {:?}", result.unwrap_or(&empty_path));
        assert_eq!(result, Some(&PathBuf::from(expected)));

        // The prebuilt index must resolve every reference identically to the
        // linear scan, including tie-breaks among same-name candidates.
        let index = VaultIndex::build(&VAULT);
        assert_eq!(
            index.lookup(input).map(PathBuf::from),
            Some(PathBuf::from(expected)),
            "index lookup diverged from linear scan for input {input:?}"
        );
    }
}
