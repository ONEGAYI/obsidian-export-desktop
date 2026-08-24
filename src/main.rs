use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use eyre::{eyre, Result};
use gumdrop::Options;
use obsidian_export::postprocessors::{filter_by_tags, softbreaks_to_hardbreaks};
use obsidian_export::{
    ExportError, ExportEvent, Exporter, FrontmatterStrategy, LinkCheckReport, LinkCheckStatus,
    LinkKind, MissingSectionStrategy, WalkOptions,
};
use serde_json::json;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the JSON Lines event schema emitted by `--progress json`. Bump on any
/// breaking change to the event format.
const JSON_EVENT_SCHEMA_VERSION: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressFormat {
    None,
    Json,
}

fn progress_format_from_str(input: &str) -> Result<ProgressFormat> {
    match input {
        "none" => Ok(ProgressFormat::None),
        "json" => Ok(ProgressFormat::Json),
        _ => Err(eyre!("must be one of: none, json")),
    }
}

#[derive(Debug, Options)]
#[allow(clippy::struct_excessive_bools)]
struct Opts {
    #[options(help = "Display program help")]
    help: bool,

    #[options(help = "Display version information")]
    version: bool,

    #[options(help = "Read notes from this source", free, required)]
    source: Option<PathBuf>,

    #[options(help = "Write notes to this destination", free, required)]
    destination: Option<PathBuf>,

    #[options(no_short, help = "Only export notes under this sub-path")]
    start_at: Option<PathBuf>,

    #[options(
        help = "Frontmatter strategy (one of: always, never, auto)",
        no_short,
        long = "frontmatter",
        parse(try_from_str = "frontmatter_strategy_from_str"),
        default = "auto"
    )]
    frontmatter_strategy: FrontmatterStrategy,

    #[options(
        no_short,
        help = "Read ignore patterns from files with this name",
        default = ".export-ignore"
    )]
    ignore_file: String,

    #[options(no_short, help = "Exclude files with this tag from the export")]
    skip_tags: Vec<String>,

    #[options(no_short, help = "Export only files with this tag")]
    only_tags: Vec<String>,

    #[options(no_short, help = "Export hidden files", default = "false")]
    hidden: bool,

    #[options(no_short, help = "Disable git integration", default = "false")]
    no_git: bool,

    #[options(no_short, help = "Don't process embeds recursively", default = "false")]
    no_recursive_embeds: bool,

    #[options(
        no_short,
        help = "Preserve the mtime of exported files",
        default = "false"
    )]
    preserve_mtime: bool,

    #[options(
        no_short,
        help = "What to do when an embed points at a missing section (one of: embed-full, skip, fail)",
        long = "missing-section",
        parse(try_from_str = "missing_section_from_str"),
        default = "skip"
    )]
    missing_section: MissingSectionStrategy,

    #[options(
        no_short,
        help = "Stop on the first failing file instead of continuing and reporting all failures at the end",
        default = "false"
    )]
    fail_fast: bool,

    #[options(
        no_short,
        help = "Progress output format (one of: none, json). json emits machine-readable JSON Lines events on stdout",
        long = "progress",
        parse(try_from_str = "progress_format_from_str"),
        default = "none"
    )]
    progress: ProgressFormat,

    #[options(
        no_short,
        help = "Convert soft line breaks to hard line breaks. This mimics Obsidian's 'Strict line breaks' setting",
        default = "false"
    )]
    hard_linebreaks: bool,
}

/// Options for `obsidian-export check`: walk the vault and verify every
/// link without writing any files.
#[derive(Debug, Options)]
#[allow(clippy::struct_excessive_bools)]
struct CheckOpts {
    #[options(help = "Display program help")]
    help: bool,

    #[options(help = "Display version information")]
    version: bool,

    #[options(
        help = "Check all links in this vault (a folder of notes)",
        free,
        required
    )]
    source: Option<PathBuf>,

    #[options(no_short, help = "Only check notes under this sub-path")]
    start_at: Option<PathBuf>,

    #[options(
        no_short,
        help = "Read ignore patterns from files with this name",
        default = ".export-ignore"
    )]
    ignore_file: String,

    #[options(no_short, help = "Check hidden files", default = "false")]
    hidden: bool,

    #[options(no_short, help = "Disable git integration", default = "false")]
    no_git: bool,

    #[options(
        no_short,
        help = "Progress output format (one of: none, json). json emits machine-readable JSON Lines events on stdout",
        long = "progress",
        parse(try_from_str = "progress_format_from_str"),
        default = "none"
    )]
    progress: ProgressFormat,
}

fn frontmatter_strategy_from_str(input: &str) -> Result<FrontmatterStrategy> {
    match input {
        "auto" => Ok(FrontmatterStrategy::Auto),
        "always" => Ok(FrontmatterStrategy::Always),
        "never" => Ok(FrontmatterStrategy::Never),
        _ => Err(eyre!("must be one of: always, never, auto")),
    }
}

fn missing_section_from_str(input: &str) -> Result<MissingSectionStrategy> {
    match input {
        "embed-full" => Ok(MissingSectionStrategy::EmbedFull),
        "skip" => Ok(MissingSectionStrategy::Skip),
        "fail" => Ok(MissingSectionStrategy::Fail),
        _ => Err(eyre!("must be one of: embed-full, skip, fail")),
    }
}

fn main() {
    // Lossy conversion avoids panicking on non-UTF-8 arguments; gumdrop's built-in
    // parse_args_default_or_exit panics on those (see the "# Panics" section of its docs).
    let argv: Vec<String> = env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    // The version flag in first position must work without the required free
    // arguments present, so it gets handled before parsing. Elsewhere, gumdrop owns
    // it (as the value of another option, or as a free argument error, just like
    // every other flag).
    if argv
        .first()
        .is_some_and(|arg| arg == "-v" || arg == "--version")
    {
        print_line(&format!("obsidian-export {VERSION}"));
        std::process::exit(0);
    }

    // The `check` subcommand is dispatched manually: gumdrop forbids a
    // `command` field in a struct that also has `free` positional arguments,
    // so matching the leading keyword ourselves is the equivalent shape.
    // When a folder named "check" exists in the working directory, the old
    // export spelling `obsidian-export check DEST` is shadowed; warn so the
    // migration path (./check) is discoverable.
    if argv.first().is_some_and(|arg| arg == "check") {
        if std::path::Path::new("check").is_dir() {
            eprintln!(
                "Warning: 'check' was interpreted as the link-check subcommand, but a \
                 directory named 'check' exists here. To export from it, use './check' \
                 as the source instead."
            );
        }
        let rest = argv.get(1..).unwrap_or(&[]);
        // Same first-position special case as the main command: version must
        // work without the required free argument present.
        if rest
            .first()
            .is_some_and(|arg| arg == "-v" || arg == "--version")
        {
            print_line(&format!("obsidian-export {VERSION}"));
            std::process::exit(0);
        }
        let check = CheckOpts::parse_args_default(rest).unwrap_or_else(|err| {
            eprintln!("Error: {err}\n\n{}", CheckOpts::usage());
            std::process::exit(2);
        });
        run_check(check);
    }

    let args = Opts::parse_args_default(&argv).unwrap_or_else(|err| {
        eprintln!("Error: {err}\n\n{}", Opts::usage());
        std::process::exit(2);
    });

    // Unlike gumdrop's default behavior of printing usage to stderr, help goes to
    // stdout, which is what virtually every other CLI does.
    if args.help {
        print_line(&format!(
            "Usage: obsidian-export [OPTIONS] SOURCE DESTINATION\n       obsidian-export check [OPTIONS] SOURCE\n\n{}",
            Opts::usage()
        ));
        std::process::exit(0);
    }
    if args.version {
        print_line(&format!("obsidian-export {VERSION}"));
        std::process::exit(0);
    }

    let root = args
        .source
        .expect("source is a required free argument enforced by gumdrop");
    let destination = args
        .destination
        .expect("destination is a required free argument enforced by gumdrop");

    let walk_options = WalkOptions {
        ignore_filename: &args.ignore_file,
        ignore_hidden: !args.hidden,
        honor_gitignore: !args.no_git,
        ..Default::default()
    };

    let mut exporter = Exporter::new(root, destination);
    exporter.frontmatter_strategy(args.frontmatter_strategy);
    exporter.process_embeds_recursively(!args.no_recursive_embeds);
    exporter.preserve_mtime(args.preserve_mtime);
    exporter.missing_section_strategy(args.missing_section);
    exporter.fail_fast(args.fail_fast);
    exporter.walk_options(walk_options);

    if args.hard_linebreaks {
        exporter.add_postprocessor(&softbreaks_to_hardbreaks);
    }

    let tags_postprocessor = filter_by_tags(args.skip_tags, args.only_tags);
    exporter.add_postprocessor(&tags_postprocessor);

    if let Some(path) = args.start_at {
        exporter.start_at(path);
    }

    if args.progress == ProgressFormat::Json {
        print_line(
            &json!({
                "type": "schema",
                "version": JSON_EVENT_SCHEMA_VERSION,
            })
            .to_string(),
        );
        let callback: obsidian_export::ExportEventCallback = Arc::new(|event: &ExportEvent| {
            if let Some(line) = event_to_json(event) {
                print_line(&line);
            }
        });
        exporter.on_event(callback);
    }

    #[allow(clippy::pattern_type_mismatch)]
    #[allow(clippy::ref_patterns)]
    #[allow(clippy::shadow_unrelated)]
    if let Err(err) = exporter.run() {
        print_run_error(err);
        std::process::exit(1);
    }
}

/// Run `obsidian-export check`: verify every link in the vault and print a
/// per-link report. Exits 0 when no links are broken, 1 when any are (or
/// the check itself fails), keeping the documented exit-code contract.
fn run_check(opts: CheckOpts) -> ! {
    if opts.help {
        print_line(&format!(
            "Usage: obsidian-export check [OPTIONS] SOURCE\n\n{}",
            CheckOpts::usage()
        ));
        std::process::exit(0);
    }
    if opts.version {
        print_line(&format!("obsidian-export {VERSION}"));
        std::process::exit(0);
    }

    let root = opts
        .source
        .expect("source is a required free argument enforced by gumdrop");
    let walk_options = WalkOptions {
        ignore_filename: &opts.ignore_file,
        ignore_hidden: !opts.hidden,
        honor_gitignore: !opts.no_git,
        ..Default::default()
    };

    // The destination is unused by check() — it never writes files — but
    // Exporter's constructor requires one; reusing the source keeps the
    // intent obvious.
    let mut exporter = Exporter::new(root.clone(), root);
    exporter.walk_options(walk_options);
    if let Some(path) = opts.start_at {
        exporter.start_at(path);
    }

    // Same emission point as exports: the schema line goes out before the
    // run starts, so a JSON consumer always knows the stream's dialect even
    // when the check itself fails before any report is produced.
    if opts.progress == ProgressFormat::Json {
        print_line(
            &json!({
                "type": "schema",
                "version": JSON_EVENT_SCHEMA_VERSION,
            })
            .to_string(),
        );
    }

    match exporter.check() {
        Ok(summary) => {
            if opts.progress == ProgressFormat::Json {
                print_line(
                    &json!({
                        "type": "check-start",
                        "files": summary.files_checked,
                    })
                    .to_string(),
                );
                for report in &summary.reports {
                    print_line(&link_report_to_json(report).to_string());
                }
                print_line(
                    &json!({
                        "type": "check-end",
                        "filesChecked": summary.files_checked,
                        "totalLinks": summary.total_links(),
                        "broken": summary.broken_links(),
                        "skipped": summary.skipped_links(),
                    })
                    .to_string(),
                );
            } else {
                for report in &summary.reports {
                    print_line(&format!(
                        "{}:{}: {} [{}]",
                        report.source.display(),
                        report.line,
                        report.status,
                        report.raw,
                    ));
                }
                print_line(&format!(
                    "\n{} file(s) checked, {} link(s) found, {} broken, {} skipped (external)",
                    summary.files_checked,
                    summary.total_links(),
                    summary.broken_links(),
                    summary.skipped_links(),
                ));
            }
            if summary.broken_links() > 0 {
                std::process::exit(1);
            }
        }
        Err(err) => {
            print_run_error(err);
            std::process::exit(1);
        }
    }
    std::process::exit(0);
}

/// Render a single [`LinkCheckReport`] as a JSON value for the
/// `check --progress json` event stream. The verdict travels as structured
/// data instead of the formatted text line, so consumers never have to parse
/// English prose to recover the target or section names.
fn link_report_to_json(report: &LinkCheckReport) -> serde_json::Value {
    // Both enums are #[non_exhaustive]: a variant added after this CLI
    // version degrades to an opaque status kind instead of dropping the
    // line (the report itself is still worth showing).
    let status = match &report.status {
        LinkCheckStatus::Ok => json!({"type": "ok"}),
        LinkCheckStatus::MissingFile { target } => {
            json!({"type": "missing-file", "target": target})
        }
        LinkCheckStatus::OutOfBounds { target } => {
            json!({"type": "out-of-bounds", "target": target})
        }
        LinkCheckStatus::MissingSection { target, section } => {
            json!({"type": "missing-section", "target": target, "section": section})
        }
        LinkCheckStatus::MissingBlock { target, block } => {
            json!({"type": "missing-block", "target": target, "block": block})
        }
        LinkCheckStatus::FileUnreadable { message } => {
            json!({"type": "file-unreadable", "message": message})
        }
        LinkCheckStatus::ExternalSkipped { url } => {
            json!({"type": "external-skipped", "url": url})
        }
        _ => json!({"type": "unknown"}),
    };
    let kind = match report.kind {
        LinkKind::WikiLink => "wiki-link",
        LinkKind::WikiEmbed => "wiki-embed",
        LinkKind::MarkdownLink => "markdown-link",
        LinkKind::MarkdownImage => "markdown-image",
        _ => "unknown",
    };
    json!({
        "type": "link-report",
        "source": report.source.display().to_string(),
        "line": report.line,
        "raw": report.raw,
        "kind": kind,
        "status": status,
    })
}

/// Print a human-readable report for a failed export run to stderr.
#[allow(clippy::pattern_type_mismatch)]
#[allow(clippy::shadow_unrelated)]
fn print_run_error(err: ExportError) {
    match err {
        ExportError::ExportCompletedWithErrors { errors } => {
            eprintln!(
                "Error: export completed with {} failing file(s):",
                errors.len()
            );
            for failed in errors {
                eprintln!("  {}: {:?}", failed.path.display(), failed.error);
            }
            eprintln!("\nHint: the first error per file is usually the root cause; re-run with --fail-fast to abort on the first failure");
        }
        ExportError::FileExportError {
            ref path,
            ref source,
        } => match &**source {
            // An arguably better way of enhancing error reports would be to construct a custom
            // `eyre::EyreHandler`, but that would require a fair amount of boilerplate and
            // reimplementation of basic reporting.
            ExportError::RecursionLimitExceeded { file_tree } => {
                eprintln!(
                    "Error: {:?}",
                    eyre!(
                        "'{}' exceeds the maximum nesting limit of embeds",
                        path.display()
                    )
                );
                eprintln!("\nFile tree:");
                for (idx, path) in file_tree.iter().enumerate() {
                    eprintln!("  {}-> {}", "  ".repeat(idx), path.display());
                }
                eprintln!("\nHint: Ensure notes are non-recursive, or specify --no-recursive-embeds to break cycles");
            }
            _ => eprintln!("Error: {:?}", eyre!(err)),
        },
        _ => eprintln!("Error: {:?}", eyre!(err)),
    }
}

/// Print a single line to stdout.
///
/// A closed stdout pipe (e.g. a GUI consumer that stopped reading — of progress events,
/// but equally of a `--version` handshake) must not turn into a panic with exit code
/// 101; exit quietly with a failure code instead, keeping the documented 0/1/2 exit
/// code contract intact. Reliable integration tests for this pipe race are hard to
/// construct, so this is covered by code review and manual verification only.
fn print_line(line: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{line}").is_err() {
        std::process::exit(1);
    }
}

/// Render an [`ExportEvent`] as a single-line JSON value for `--progress json`.
/// Returns `None` for future event variants unknown to this CLI version.
fn event_to_json(event: &ExportEvent) -> Option<String> {
    let value = match event {
        ExportEvent::Start { total } => json!({"type": "start", "total": total}),
        ExportEvent::FileDone { path } => json!({
            "type": "file-done",
            "path": path.display().to_string(),
        }),
        ExportEvent::FileSkipped { path } => json!({
            "type": "file-skipped",
            "path": path.display().to_string(),
        }),
        ExportEvent::FileFailed { path, message } => json!({
            "type": "file-failed",
            "path": path.display().to_string(),
            "message": message,
        }),
        ExportEvent::Warning { path, message } => json!({
            "type": "warning",
            "path": path.as_ref().map(|p| p.display().to_string()),
            "message": message,
        }),
        ExportEvent::End { failed } => json!({
            "type": "end",
            "failed": failed.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        }),
        _ => return None,
    };
    Some(value.to_string())
}
