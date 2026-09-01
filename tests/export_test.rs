#![allow(clippy::shadow_unrelated)]

use std::fs::read_to_string;
#[cfg(not(target_os = "windows"))]
use std::fs::{create_dir, set_permissions, File, Permissions};
#[cfg(not(target_os = "windows"))]
use std::io::prelude::*;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use obsidian_export::postprocessors::obsidian_comments;
use obsidian_export::{
    CommentsMode,
    ExportError,
    ExportEvent,
    Exporter,
    FrontmatterStrategy,
    MissingSectionStrategy,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use walkdir::WalkDir;

/// Compare every file under the temporary export dir against the golden
/// tree `tests/testdata/expected/<golden>/`.
fn assert_matches_golden(tmp_dir: &TempDir, golden: &str) {
    let walker = WalkDir::new(format!("tests/testdata/expected/{golden}/"))
        // Without sorting here, different test runs may trigger the first assertion failure in
        // unpredictable order.
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter();
    for entry in walker {
        let entry = entry.unwrap();
        if entry.metadata().unwrap().is_dir() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        let expected = read_to_string(entry.path()).unwrap_or_else(|_| {
            panic!(
                "failed to read {} from testdata/expected/{golden}/",
                entry.path().display()
            )
        });
        let actual = read_to_string(tmp_dir.path().join(PathBuf::from(&filename)))
            .unwrap_or_else(|_| panic!("failed to read {} from temporary exportdir", filename));

        assert_eq!(
            expected, actual,
            "{} does not have expected content",
            filename
        );
    }
}

#[test]
fn test_main_variants_with_default_options() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let walker = WalkDir::new("tests/testdata/expected/main-samples/")
        // Without sorting here, different test runs may trigger the first assertion failure in
        // unpredictable order.
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter();
    for entry in walker {
        let entry = entry.unwrap();
        if entry.metadata().unwrap().is_dir() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        let expected = read_to_string(entry.path()).unwrap_or_else(|_| {
            panic!(
                "failed to read {} from testdata/expected/main-samples/",
                entry.path().display()
            )
        });
        let actual = read_to_string(tmp_dir.path().join(PathBuf::from(&filename)))
            .unwrap_or_else(|_| panic!("failed to read {} from temporary exportdir", filename));

        assert_eq!(
            expected, actual,
            "{} does not have expected content",
            filename
        );
    }
}

#[test]
fn test_frontmatter_never() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.frontmatter_strategy(FrontmatterStrategy::Never);
    exporter.run().expect("exporter returned error");

    let expected = "Note with frontmatter.\n";
    let actual = read_to_string(
        tmp_dir
            .path()
            .join(PathBuf::from("note-with-frontmatter.md")),
    )
    .unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn test_frontmatter_always() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.frontmatter_strategy(FrontmatterStrategy::Always);
    exporter.run().expect("exporter returned error");

    // Note without frontmatter should have empty frontmatter added.
    let expected = "---\n---\n\nNote without frontmatter.\n";
    let actual = read_to_string(
        tmp_dir
            .path()
            .join(PathBuf::from("note-without-frontmatter.md")),
    )
    .unwrap();
    assert_eq!(expected, actual);

    // Note with frontmatter should remain untouched.
    let expected = "---\nFoo: bar\n---\n\nNote with frontmatter.\n";
    let actual = read_to_string(
        tmp_dir
            .path()
            .join(PathBuf::from("note-with-frontmatter.md")),
    )
    .unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_exclude() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let excluded_note = tmp_dir.path().join(PathBuf::from("excluded-note.md"));
    assert!(
        !excluded_note.exists(),
        "exluded-note.md was found in tmpdir, but should be absent due to .export-ignore rules"
    );
}

#[test]
fn test_single_file_to_dir() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    Exporter::new(
        PathBuf::from("tests/testdata/input/single-file/note.md"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .unwrap();

    assert_eq!(
        read_to_string("tests/testdata/expected/single-file/note.md").unwrap(),
        read_to_string(tmp_dir.path().join(PathBuf::from("note.md"))).unwrap(),
    );
}

#[test]
fn test_single_file_to_file() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().join(PathBuf::from("export.md"));

    Exporter::new(
        PathBuf::from("tests/testdata/input/single-file/note.md"),
        dest.clone(),
    )
    .run()
    .unwrap();

    assert_eq!(
        read_to_string("tests/testdata/expected/single-file/note.md").unwrap(),
        read_to_string(&dest).unwrap(),
    );
}

#[test]
fn test_start_at_subdir() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/start-at/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.start_at(PathBuf::from("tests/testdata/input/start-at/subdir"));
    exporter.run().unwrap();

    let expected = read_to_string("tests/testdata/expected/start-at/subdir/Note B.md").unwrap();

    assert_eq!(
        expected,
        read_to_string(tmp_dir.path().join(PathBuf::from("Note B.md"))).unwrap(),
    );
}

#[test]
fn test_start_at_file_within_subdir_destination_is_dir() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/start-at/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.start_at(PathBuf::from(
        "tests/testdata/input/start-at/subdir/Note B.md",
    ));
    exporter.run().unwrap();

    let expected =
        read_to_string("tests/testdata/expected/start-at/single-file/Note B.md").unwrap();

    assert_eq!(
        expected,
        read_to_string(tmp_dir.path().join(PathBuf::from("Note B.md"))).unwrap(),
    );
}

#[test]
fn test_start_at_file_within_subdir_destination_is_file() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().join(PathBuf::from("note.md"));
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/start-at/"),
        dest.clone(),
    );
    exporter.start_at(PathBuf::from(
        "tests/testdata/input/start-at/subdir/Note B.md",
    ));
    exporter.run().unwrap();

    let expected =
        read_to_string("tests/testdata/expected/start-at/single-file/Note B.md").unwrap();
    assert_eq!(expected, read_to_string(dest).unwrap(),);
}

#[test]
fn test_not_existing_source() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let err = Exporter::new(
        PathBuf::from("tests/testdata/no-such-file.md"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .unwrap_err();

    match err {
        ExportError::PathDoesNotExist { .. } => {}
        _ => panic!("Wrong error variant: {:?}", err),
    }
}

#[test]
fn test_not_existing_destination_with_source_dir() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let err = Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf().join("does-not-exist"),
    )
    .run()
    .unwrap_err();

    match err {
        ExportError::PathDoesNotExist { .. } => {}
        _ => panic!("Wrong error variant: {:?}", err),
    }
}

#[test]
// This test ensures that when source is a file, but destination points to a
// regular file inside of a non-existent directory, an error is raised instead
// of that directory path being created (like `mkdir -p`)
fn test_not_existing_destination_with_source_file() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let err = Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/obsidian-wikilinks.md"),
        tmp_dir.path().to_path_buf().join("subdir/does-not-exist"),
    )
    .run()
    .unwrap_err();

    match err {
        ExportError::PathDoesNotExist { .. } => {}
        _ => panic!("Wrong error variant: {:?}", err),
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_source_no_permissions() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let src = tmp_dir.path().to_path_buf().join("source.md");
    let dest = tmp_dir.path().to_path_buf().join("dest.md");

    let mut file = File::create(&src).unwrap();
    file.write_all(b"Foo").unwrap();
    drop(file);
    set_permissions(&src, Permissions::from_mode(0o000)).unwrap();

    // Bind the error before matching: the temporary chain inside a tail
    // expression trips `tail_expr_drop_order` (the Rust 2024 drop-order
    // change) against the still-live `tmp_dir`.
    let err = Exporter::new(src, dest).run().unwrap_err();
    match err {
        ExportError::FileExportError { source, .. } => match *source {
            ExportError::ReadError { .. } => {}
            _ => panic!("Wrong error variant for source, got: {:?}", source),
        },
        err => panic!("Wrong error variant: {:?}", err),
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_dest_no_permissions() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let src = tmp_dir.path().to_path_buf().join("source.md");
    let dest = tmp_dir.path().to_path_buf().join("dest");

    let mut file = File::create(&src).unwrap();
    file.write_all(b"Foo").unwrap();
    drop(file);

    create_dir(&dest).unwrap();
    set_permissions(&dest, Permissions::from_mode(0o555)).unwrap();

    // Same tail-expression pattern as above: bind before matching.
    let err = Exporter::new(src, dest).run().unwrap_err();
    match err {
        ExportError::FileExportError { source, .. } => match *source {
            ExportError::WriteError { .. } => {}
            _ => panic!("Wrong error variant for source, got: {:?}", source),
        },
        err => panic!("Wrong error variant: {:?}", err),
    }
}

#[test]
fn test_infinite_recursion() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let err = Exporter::new(
        PathBuf::from("tests/testdata/input/infinite-recursion/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .unwrap_err();

    // With error aggregation (the default), each file involved in the cycle fails
    // individually with RecursionLimitExceeded; the export as a whole reports the
    // collected failures instead of aborting on the first one.
    match err {
        ExportError::ExportCompletedWithErrors { errors } => {
            assert!(!errors.is_empty());
            for failed in &errors {
                match &failed.error {
                    ExportError::FileExportError { source, .. } => {
                        assert!(
                            matches!(**source, ExportError::RecursionLimitExceeded { .. }),
                            "Wrong error variant for source, got: {:?}",
                            source
                        );
                    }
                    _ => panic!("Wrong error variant"),
                }
            }
        }
        _ => panic!("Wrong error variant"),
    }
}

#[test]
fn test_no_recursive_embeds() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/infinite-recursion/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.process_embeds_recursively(false);
    exporter.run().expect("exporter returned error");

    assert_eq!(
        read_to_string("tests/testdata/expected/infinite-recursion/Note A.md").unwrap(),
        read_to_string(tmp_dir.path().join(PathBuf::from("Note A.md"))).unwrap(),
    );
}

#[test]
fn test_preserve_mtime() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.preserve_mtime(true);
    exporter.run().expect("exporter returned error");

    let src = "tests/testdata/input/main-samples/obsidian-wikilinks.md";
    let dest = tmp_dir.path().join(PathBuf::from("obsidian-wikilinks.md"));
    let src_meta = std::fs::metadata(src).unwrap();
    let dest_meta = std::fs::metadata(dest).unwrap();

    assert_eq!(src_meta.modified().unwrap(), dest_meta.modified().unwrap());
}

#[test]
fn test_no_preserve_mtime() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/main-samples/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.preserve_mtime(false);
    exporter.run().expect("exporter returned error");

    let src = "tests/testdata/input/main-samples/obsidian-wikilinks.md";
    let dest = tmp_dir.path().join(PathBuf::from("obsidian-wikilinks.md"));
    let src_meta = std::fs::metadata(src).unwrap();
    let dest_meta = std::fs::metadata(dest).unwrap();

    assert_ne!(src_meta.modified().unwrap(), dest_meta.modified().unwrap());
}

#[test]
fn test_non_ascii_filenames() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/non-ascii/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let walker = WalkDir::new("tests/testdata/expected/non-ascii/")
        // Without sorting here, different test runs may trigger the first assertion failure in
        // unpredictable order.
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter();
    for entry in walker {
        let entry = entry.unwrap();
        if entry.metadata().unwrap().is_dir() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        let expected = read_to_string(entry.path()).unwrap_or_else(|_| {
            panic!(
                "failed to read {} from testdata/expected/non-ascii/",
                entry.path().display()
            )
        });
        let actual = read_to_string(tmp_dir.path().join(PathBuf::from(&filename)))
            .unwrap_or_else(|_| panic!("failed to read {} from temporary exportdir", filename));

        assert_eq!(
            expected, actual,
            "{} does not have expected content",
            filename
        );
    }
}

#[test]
fn test_start_at_outside_root_errors() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/start-at/"),
        tmp_dir.path().to_path_buf(),
    );
    // A start-at path outside the vault root used to silently export zero files.
    exporter.start_at(PathBuf::from("tests/testdata/input/non-ascii"));
    match exporter.run() {
        Err(ExportError::StartAtNotUnderRoot { .. }) => (),
        _ => panic!("expected StartAtNotUnderRoot"),
    }
}

#[test]
fn test_start_at_nonexistent_errors() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/start-at/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.start_at(PathBuf::from(
        "tests/testdata/input/start-at/no-such-subdir",
    ));
    match exporter.run() {
        Err(ExportError::PathDoesNotExist { .. }) => (),
        _ => panic!("expected PathDoesNotExist"),
    }
}

#[test]
fn test_error_aggregation_continues_by_default() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let err = Exporter::new(
        PathBuf::from("tests/testdata/input/mixed-health/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .unwrap_err();

    // One note has broken frontmatter; the rest of the vault still exports.
    let failed_paths = match &err {
        ExportError::ExportCompletedWithErrors { errors } => {
            assert_eq!(errors.len(), 1);
            errors.iter().map(|f| f.path.clone()).collect::<Vec<_>>()
        }
        _ => panic!("expected ExportCompletedWithErrors"),
    };
    assert!(
        failed_paths
            .first()
            .is_some_and(|p| p.ends_with("bad-frontmatter.md")),
        "unexpected failed paths: {:?}",
        failed_paths
    );
    assert!(
        tmp_dir.path().join("good.md").exists(),
        "healthy notes should still be exported"
    );
    assert!(
        tmp_dir.path().join("broken-link.md").exists(),
        "notes with broken links are warnings, not failures"
    );
}

#[test]
fn test_fail_fast_aborts_immediately() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/mixed-health/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.fail_fast(true);
    let err = exporter.run().unwrap_err();
    match &err {
        ExportError::FileExportError { source, .. } => {
            assert!(
                matches!(**source, ExportError::FrontMatterDecodeError { .. }),
                "expected FrontMatterDecodeError, got {:?}",
                source
            );
        }
        _ => panic!("expected FileExportError"),
    }
}

#[test]
fn test_event_stream_reports_progress_and_warnings() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let events: Arc<Mutex<Vec<ExportEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/mixed-health/"),
        tmp_dir.path().to_path_buf(),
    );
    let sink = Arc::clone(&events);
    exporter.on_event(Arc::new(move |event: &ExportEvent| {
        sink.lock()
            .expect("event sink poisoned")
            .push(event.clone());
    }));
    exporter.run().unwrap_err();

    let events = events.lock().expect("event sink poisoned").clone();
    assert!(matches!(
        events.first(),
        Some(ExportEvent::Start { total: 3 })
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ExportEvent::FileDone { path } if path.ends_with("good.md"))),
        "missing FileDone for good.md"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ExportEvent::FileFailed { path, .. } if path.ends_with("bad-frontmatter.md"))),
        "missing FileFailed for bad-frontmatter.md"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            ExportEvent::Warning {
                path: Some(path),
                ..
            } if path.ends_with("broken-link.md")
        )),
        "missing Warning event for broken link"
    );
    match events.last() {
        Some(ExportEvent::End { failed }) => assert_eq!(failed.len(), 1),
        _ => panic!("expected End event"),
    }
}

#[test]
fn test_chinese_section_anchor() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/chinese-anchor/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // Anchors for CJK headings must be preserved as-is rather than transliterated,
    // otherwise section links point nowhere on renderers like GitHub. Fullwidth
    // punctuation is stripped from the anchor (matching GitHub/VS Code), while the
    // link text keeps the original spelling.
    let expected = concat!(
        "链接到 [target > 中文标题](target.md#中文标题) 的引用。\n\n",
        "也链接到 [target > Mixed 混合 Heading](target.md#mixed-混合-heading)。\n\n",
        "再链接全角标点标题：[target > 总纲：三份形态，两个断口](target.md#总纲三份形态两个断口)",
        " 与 [target > 断口-a：入库前，输入已非原话](target.md#断口-a入库前输入已非原话)。\n",
    );
    let actual = read_to_string(tmp_dir.path().join(PathBuf::from("note.md"))).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_section_matching_variants() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/section-variants/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // Headings containing inline code must match section references by their plain
    // text, mirroring how Obsidian resolves such links.
    let actual = read_to_string(tmp_dir.path().join("note-code-heading.md")).unwrap();
    assert!(
        actual.contains("## `code` heading"),
        "embed keeps the heading, got: {:?}",
        actual
    );
    assert!(
        actual.contains("code section content."),
        "embed keeps the section content, got: {:?}",
        actual
    );
    assert!(
        actual.contains("(target.md#code-heading)"),
        "link anchor resolves, got: {:?}",
        actual
    );

    // A same-named heading nested deeper than the target must not restart the
    // section: the embed runs from the first match to the end of the note.
    let actual = read_to_string(tmp_dir.path().join("note-nested-duplicate.md")).unwrap();
    assert!(
        actual.contains("outer content."),
        "embed starts at the first matching heading, got: {:?}",
        actual
    );
    assert!(
        actual.contains("### Target"),
        "embed includes the nested same-named heading, got: {:?}",
        actual
    );
}

#[test]
fn test_heading_wikilink_section_embeds() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/heading-wikilink/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // A heading containing a wikilink (`## [[mid]]`) aggregates by its display
    // text, so `![[target#mid]]` resolves it (as it did before the raw/expand
    // split); inside the embedded slice the wikilink still expands normally.
    // Section references cannot contain `]` (the wikilink grammar forbids it),
    // so literal-bracket headings are exercised as content: the mixed heading
    // `### [WIP] and [[mid]]` must keep its literal prefix while its wikilink
    // expands, and must not disturb the section boundaries.
    let actual = read_to_string(tmp_dir.path().join("note.md")).unwrap();
    assert!(
        actual.contains("[mid](mid.md)"),
        "wikilink heading embeds and expands its link, got: {:?}",
        actual
    );
    assert!(
        actual.contains("wikilink heading content."),
        "wikilink-heading section content kept, got: {:?}",
        actual
    );
    assert!(
        actual.contains("mixed heading content."),
        "mixed literal-bracket heading kept as section content, got: {:?}",
        actual
    );
    assert!(
        actual.contains("WIP"),
        "literal bracket prefix preserved, got: {:?}",
        actual
    );
    assert!(
        !actual.contains("after content."),
        "same-level heading after the section terminates the embed, got: {:?}",
        actual
    );

    // The source note itself renders the heading wikilink as a plain link.
    let target = read_to_string(tmp_dir.path().join("target.md")).unwrap();
    assert!(
        target.contains("## [mid](mid.md)"),
        "heading wikilink expands in the source note, got: {:?}",
        target
    );
}

#[test]
fn test_numbered_section_reference_and_embed() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/numbered-section/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // `N. Title` headings resolve from section references verbatim: the
    // list-marker-like prefix is heading text, not markup, so the embed
    // slices the right section and the generated anchor keeps the ordinal.
    let actual = read_to_string(tmp_dir.path().join(PathBuf::from("note.md"))).unwrap();
    assert!(
        actual.contains("numbered content."),
        "embed slices the numbered section, got: {:?}",
        actual
    );
    assert!(
        !actual.contains("after content."),
        "next same-level heading terminates the embed, got: {:?}",
        actual
    );
    assert!(
        actual.contains("(target.md#5-numbered-section)"),
        "link anchor keeps the numbered prefix, got: {:?}",
        actual
    );
}

#[test]
fn test_numeric_image_size_label_falls_back_to_filename() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/image-size/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // Obsidian's size syntax (`![[img.png|300]]`) surfaces as a purely numeric label;
    // plain Markdown has no image sizing, so the filename is used as alt text instead
    // of a bare number.
    let expected = "![img.png](img.png)\n";
    let actual = read_to_string(tmp_dir.path().join("note.md")).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_embedded_images_with_relative_paths() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/relative-references/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("notes/note.md")).unwrap();

    // Obsidian resolves wikilinks with explicit relative components (`./`, `../`)
    // against the containing note's directory; image URLs use forward slashes so
    // plain-Markdown renderers resolve them.
    assert!(
        actual.contains("![../assets/diagram.svg](../assets/diagram.svg)"),
        "parent-relative image embed resolves, got: {:?}",
        actual
    );
    // Non-ASCII filenames resolve the same way, kept verbatim in the URL (only
    // characters that would break a Markdown link destination are escaped).
    assert!(
        actual.contains("![../assets/图.svg](../assets/图.svg)"),
        "parent-relative non-ASCII image embed resolves, got: {:?}",
        actual
    );
    assert!(
        actual.contains("![./same-dir.svg](same-dir.svg)"),
        "current-dir image embed resolves, got: {:?}",
        actual
    );
    // References without relative components keep the vault suffix-match behavior,
    // with forward slashes in the URL (relative to the containing note).
    assert!(
        actual.contains("![assets/diagram.svg](../assets/diagram.svg)"),
        "suffix-matched image embed keeps forward slashes, got: {:?}",
        actual
    );
    // References whose relative path escapes the vault must not resolve to anything.
    assert!(
        !actual.contains("escape.svg]("),
        "out-of-vault embed stays unresolved, got: {:?}",
        actual
    );
}

#[test]
fn test_missing_section_skip_by_default() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/missing-sections/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // A direct missing-section embed collapses to nothing (leaving the blank lines of its
    // paragraph, mirroring how missing-file embeds behave); surrounding content is kept.
    let expected = "嵌入缺失章节：\n\n\n\n后文。\n";
    let actual = read_to_string(tmp_dir.path().join("note-embed-missing.md")).unwrap();
    assert_eq!(expected, actual);

    // Outer embed missing: only the embed disappears, the rest of the note survives.
    let expected = "外层缺失嵌入：\n\n\n\n尾注。\n";
    let actual = read_to_string(tmp_dir.path().join("note-outer-missing.md")).unwrap();
    assert_eq!(expected, actual);

    // Outer embed hits, inner embed (inside the embedded note) misses: the strategy
    // applies per nesting level, so the inner embed is dropped but the outer content
    // around it is preserved.
    let expected = "外层命中、内层缺失：\n\n# Real\n\n内文开头。\n\n\n\n内文结尾。\n";
    let actual = read_to_string(tmp_dir.path().join("note-inner-missing.md")).unwrap();
    assert_eq!(expected, actual);

    // Block references never match a heading and follow the same strategy.
    let expected = "块引用嵌入：\n\n\n";
    let actual = read_to_string(tmp_dir.path().join("note-block-ref.md")).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_missing_section_embed_full() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/missing-sections/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.missing_section_strategy(MissingSectionStrategy::EmbedFull);
    exporter.run().expect("exporter returned error");

    // Upstream behavior: the entire note is embedded when the section is missing.
    let expected = "嵌入缺失章节：\n\n# Real\n\nreal content.\n\n后文。\n";
    let actual = read_to_string(tmp_dir.path().join("note-embed-missing.md")).unwrap();
    assert_eq!(expected, actual);

    // Outer embed missing: the strategy applies per nesting level, so the outer embed
    // pulls in the whole nested note, whose own missing-section embed pulls in the
    // whole target note. No section cut happens here, so nothing is truncated.
    let expected =
        "外层缺失嵌入：\n\n# Real\n\n内文开头。\n\n# Real\n\nreal content.\n\n内文结尾。\n\n尾注。\n";
    let actual = read_to_string(tmp_dir.path().join("note-outer-missing.md")).unwrap();
    assert_eq!(expected, actual);

    // Outer embed hits, inner embed missing. The section cut happens on the
    // embedded note's own events before nested embeds expand, so the outer
    // note's own content ("内文结尾") survives even though the inner embed
    // (EmbedFull) pulls in a same-level "# Real" heading.
    let expected =
        "外层命中、内层缺失：\n\n# Real\n\n内文开头。\n\n# Real\n\nreal content.\n\n内文结尾。\n";
    let actual = read_to_string(tmp_dir.path().join("note-inner-missing.md")).unwrap();
    assert_eq!(expected, actual);

    // Block references embed the full note as well.
    let expected = "块引用嵌入：\n\n# Real\n\nreal content.\n";
    let actual = read_to_string(tmp_dir.path().join("note-block-ref.md")).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_embed_section_cut_before_expansion() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/embed-order/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    // The section cut must happen on the embedded note's own events, before
    // its nested embeds are expanded: a heading pulled in by the inner embed
    // (`# Sub`, same level as the outer `# Real`) must not terminate the
    // outer section, so the outer note's own "after." paragraph survives.
    let expected = "# Real\n\nbefore.\n\n# Sub\n\nsub content.\n\nafter.\n";
    let actual = read_to_string(tmp_dir.path().join("root.md")).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_block_refs_embed_located_block() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/block-refs/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("embeds.md")).unwrap();
    // Trailing-id paragraph block; the id itself is stripped from the output.
    assert!(
        actual.contains("行尾块：First paragraph block"),
        "paragraph block located: {}",
        actual
    );
    assert!(
        !actual.contains(" ^para1"),
        "trailing block id stripped: {}",
        actual
    );
    // Standalone id line marks the block above it.
    assert!(
        actual.contains("独立行块：Standalone-id block above."),
        "standalone id marks preceding block: {}",
        actual
    );
    assert!(
        !actual.contains("standalone1"),
        "standalone id not leaked: {}",
        actual
    );
    // Id on a list bullet resolves to that item only (the bullet glyph and
    // spacing are renderer details, so match the text).
    assert!(
        actual.contains("list item two"),
        "list item block located: {}",
        actual
    );
    assert!(
        !actual.contains("list item one"),
        "sibling item excluded: {}",
        actual
    );
    // Id at the end of a quote block resolves to the whole quote (the quote
    // starts on its own rendered line after the inline label).
    assert!(
        actual.contains("> quoted block line"),
        "quote block located with its quote context: {}",
        actual
    );
    assert!(
        actual.contains("> quoted end"),
        "quote second line kept: {}",
        actual
    );
    assert!(
        !actual.contains("^quote1"),
        "quote block id stripped: {}",
        actual
    );
    // Unknown id falls back to the missing-section strategy (Skip by default).
    assert!(
        !actual.contains("nope"),
        "unmatched block id collapses: {}",
        actual
    );
}

#[test]
fn test_same_file_section_and_block_embeds() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/block-refs/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("self.md")).unwrap();
    assert!(
        actual.contains("beta 引用 alpha 块：alpha 内容段"),
        "same-file block embed splices the block: {}",
        actual
    );
    // The id definition in the source note itself is kept (upstream
    // behavior); only the embedded copy must have it stripped.
    assert!(
        !actual.contains("beta 引用 alpha 块：alpha 内容段 ^"),
        "same-file block id stripped from the embedded copy: {}",
        actual
    );
    assert!(
        actual.contains("beta 引用 section：## Alpha"),
        "same-file section embed splices the section: {}",
        actual
    );
    assert!(
        actual.contains("alpha 内容段"),
        "section content present: {}",
        actual
    );
}

#[test]
fn test_same_file_self_referencing_block_terminates() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    // A block embedding itself expands exactly once: the embedded copy has
    // its id marker stripped, so the inner reference can't resolve anymore
    // and degrades per the missing-section strategy (Skip: collapses to
    // empty). The cycle terminates without hitting the recursion limit.
    Exporter::new(
        PathBuf::from("tests/testdata/input/block-refs-self-loop/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("self-loop.md")).unwrap();
    let occurrences = actual.matches("这一段含自引用").count();
    assert_eq!(
        occurrences, 2,
        "label appears once in the source text plus once from the single expansion: {}",
        actual
    );
}

#[test]
fn test_wikilink_formatting_markers_preserved() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/formatting-refs/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("note.md")).unwrap();
    // The `__dunder__` spelling survives into file lookup, section matching
    // and the generated anchor (matching GitHub-style heading anchors). The
    // embedded heading renders as `## **dunder**` — that's just how
    // pulldown-cmark-to-cmark renders the Strong event of `## __dunder__`.
    assert!(
        actual.contains("## **dunder**"),
        "section with underscore formatting matched: {}",
        actual
    );
    assert!(
        actual.contains("dunder section content"),
        "section content spliced: {}",
        actual
    );
    assert!(
        actual.contains("target.md#__dunder__"),
        "anchor keeps the underscores: {}",
        actual
    );
    assert!(
        actual.contains("__file__.md"),
        "filename with underscores resolves: {}",
        actual
    );
}

#[test]
fn test_escaped_pipe_wikilinks_resolve_inside_tables() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/escaped-pipe-refs/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("note.md")).unwrap();
    // Obsidian requires `\|` for aliased wikilinks inside Markdown tables; the
    // parser must treat it as the separator instead of leaving the backslash
    // in the filename (which would break file lookup and degrade the link to
    // italic text).
    assert!(
        actual.contains("[Alias](target.md)"),
        "escaped-pipe alias link resolves: {}",
        actual
    );
    assert!(
        actual.contains("[Head Alias](target.md#heading)"),
        "escaped-pipe section link resolves: {}",
        actual
    );
    assert!(
        !actual.contains("*Alias*") && !actual.contains("*Head Alias*"),
        "no degraded italic fallback for escaped-pipe links: {}",
        actual
    );
}

#[test]
fn test_block_refs_edge_cases() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");

    Exporter::new(
        PathBuf::from("tests/testdata/input/block-refs-edge/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join("note-edge.md")).unwrap();
    // An id inside a code block (EOF block without trailing newline) must not
    // become a candidate: code content is never spliced or rewritten.
    assert!(
        !actual.contains("plain"),
        "code block id not treated as a block marker: {}",
        actual
    );
    // An id on a paragraph inside a *nested* quote resolves to the innermost
    // quote block, not the whole outer quote.
    assert!(
        actual.contains("inner"),
        "nested quote block located: {}",
        actual
    );
    assert!(
        !actual.contains("outer line"),
        "outer quote excluded: {}",
        actual
    );
    // An id on a list item inside a quote keeps its quote context.
    assert!(
        actual.contains("quoted item"),
        "quoted list item located: {}",
        actual
    );
    assert!(
        actual.contains('>'),
        "quote prefix kept for embedded list item: {}",
        actual
    );

    // A same-file section embed whose target contains itself must degrade to
    // a link instead of failing the whole file with RecursionLimitExceeded.
    let actual = read_to_string(tmp_dir.path().join("self-section-loop.md")).unwrap();
    assert!(
        actual.contains("内含自环：→"),
        "self-referencing section degrades to a link: {}",
        actual
    );
}

#[test]
fn test_missing_section_fail() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/missing-sections/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.missing_section_strategy(MissingSectionStrategy::Fail);

    match exporter.run() {
        Err(ExportError::ExportCompletedWithErrors { errors }) => {
            // Exactly the five notes carrying a missing-section embed must fail (in
            // some order — the export runs in parallel); target.md must survive.
            let mut failed_files: Vec<String> = errors
                .iter()
                .map(|failed| {
                    failed
                        .path
                        .file_name()
                        .expect("failed path has a filename")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            failed_files.sort();
            assert_eq!(
                failed_files,
                vec![
                    "note-block-ref.md",
                    "note-embed-missing.md",
                    "note-inner-missing.md",
                    "note-nested-inner.md",
                    "note-outer-missing.md",
                ],
                "expected exactly the five embedding notes to fail, got {:?}",
                errors
            );
            // Block references (`#^block-id`) never match a heading, so they fail
            // through the same SectionNotFound path.
            assert!(
                errors.iter().any(|failed| matches!(
                    &failed.error,
                    ExportError::FileExportError { source, .. }
                        if matches!(
                            &**source,
                            ExportError::SectionNotFound { section, .. } if section == "^blockid"
                        )
                )),
                "expected a SectionNotFound for the block reference, got {:?}",
                errors
            );
        }
        Err(err) => panic!("expected ExportCompletedWithErrors, got {:?}", err),
        Ok(()) => panic!("expected export to fail with SectionNotFound"),
    }
}

#[test]
fn test_same_filename_different_directories() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    Exporter::new(
        PathBuf::from("tests/testdata/input/same-filename-different-directories"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .unwrap();

    let expected =
        read_to_string("tests/testdata/expected/same-filename-different-directories/Note.md")
            .unwrap();

    let actual = read_to_string(tmp_dir.path().join(PathBuf::from("Note.md"))).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_comments_convert() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let comments = obsidian_comments(CommentsMode::Convert);
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/comments/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.add_postprocessor(&comments);
    exporter.run().expect("exporter returned error");
    assert_matches_golden(&tmp_dir, "comments");
}

#[test]
fn test_comments_strip() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let comments = obsidian_comments(CommentsMode::Strip);
    let mut exporter = Exporter::new(
        PathBuf::from("tests/testdata/input/comments/"),
        tmp_dir.path().to_path_buf(),
    );
    exporter.add_postprocessor(&comments);
    exporter.run().expect("exporter returned error");
    assert_matches_golden(&tmp_dir, "comments-strip");
}

#[test]
fn test_comments_kept_by_default() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    Exporter::new(
        PathBuf::from("tests/testdata/input/comments/"),
        tmp_dir.path().to_path_buf(),
    )
    .run()
    .expect("exporter returned error");

    let actual = read_to_string(tmp_dir.path().join(PathBuf::from("Note.md"))).unwrap();
    // assert! messages are plain literals on edition 2018 (no format
    // expansion), so the failing value is left to the assertion context.
    assert!(
        actual.contains("%%inline%%"),
        "without the postprocessor, comments must stay verbatim"
    );
    assert!(
        !actual.contains("<!--"),
        "default export must not emit HTML comments"
    );
}
