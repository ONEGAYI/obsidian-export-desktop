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

use obsidian_export::{
    ExportError,
    ExportEvent,
    Exporter,
    FrontmatterStrategy,
    MissingSectionStrategy,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use walkdir::WalkDir;

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

    let expected =
        read_to_string("tests/testdata/expected/start-at/subdir/Note B.md").unwrap();

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
    set_permissions(&src, Permissions::from_mode(0o000)).unwrap();

    match Exporter::new(src, dest).run().unwrap_err() {
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

    create_dir(&dest).unwrap();
    set_permissions(&dest, Permissions::from_mode(0o555)).unwrap();

    match Exporter::new(src, dest).run().unwrap_err() {
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
    // otherwise section links point nowhere on renderers like GitHub.
    let expected = "链接到 [target > 中文标题](target.md#中文标题) 的引用。\n\n也链接到 [target > Mixed 混合 Heading](target.md#mixed-混合-heading)。\n";
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
    let expected = "外层命中、内层缺失：\n\n# Real\n\n内文开头。\n\n# Real\n\nreal content.\n\n内文结尾。\n";
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
