//! A collection of officially maintained [postprocessors][crate::Postprocessor].

use pulldown_cmark::Event;
use serde_yaml::Value;

use super::{comments, CommentsMode, Context, MarkdownEvents, PostprocessorResult};

/// This postprocessor converts all soft line breaks to hard line breaks. Enabling this mimics
/// Obsidian's _'Strict line breaks'_ setting.
pub fn softbreaks_to_hardbreaks(
    _context: &mut Context,
    events: &mut MarkdownEvents<'_>,
) -> PostprocessorResult {
    for event in events.iter_mut() {
        if event == &Event::SoftBreak {
            *event = Event::HardBreak;
        }
    }
    PostprocessorResult::Continue
}

/// Rewrite Obsidian `%%...%%` comments according to `mode`.
///
/// Comments are recognized everywhere Obsidian recognizes them, except
/// inside code blocks, inline code, math, tables and link labels. A comment
/// spanning blank lines or other block boundaries is emitted as an HTML
/// block so the surrounding container structure stays balanced. See the
/// [`comments`](crate::comments) module for the full recognition contract.
///
/// With [`CommentsMode::Keep`] this is a no-op.
pub fn obsidian_comments(
    mode: CommentsMode,
) -> impl Fn(&mut Context, &mut MarkdownEvents<'_>) -> PostprocessorResult {
    move |_context: &mut Context, events: &mut MarkdownEvents<'_>| -> PostprocessorResult {
        comments::rewrite_events(events, mode);
        PostprocessorResult::Continue
    }
}

pub fn filter_by_tags(
    skip_tags: Vec<String>,
    only_tags: Vec<String>,
) -> impl Fn(&mut Context, &mut MarkdownEvents<'_>) -> PostprocessorResult {
    move |context: &mut Context, _events: &mut MarkdownEvents<'_>| -> PostprocessorResult {
        match context.frontmatter.get("tags") {
            None => filter_by_tags_(&[], &skip_tags, &only_tags),
            Some(Value::Sequence(tags)) => filter_by_tags_(tags, &skip_tags, &only_tags),
            // Obsidian also accepts a scalar or comma-separated string (`tags: foo` or
            // `tags: foo, bar`); normalize those to a list before filtering.
            Some(Value::String(tags)) => {
                let tags: Vec<Value> = tags
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(|tag| Value::String(tag.to_owned()))
                    .collect();
                filter_by_tags_(&tags, &skip_tags, &only_tags)
            }
            _ => PostprocessorResult::Continue,
        }
    }
}

fn filter_by_tags_(
    tags: &[Value],
    skip_tags: &[String],
    only_tags: &[String],
) -> PostprocessorResult {
    let skip = skip_tags
        .iter()
        .any(|tag| tags.contains(&Value::String(tag.clone())));
    let include = only_tags.is_empty()
        || only_tags
            .iter()
            .any(|tag| tags.contains(&Value::String(tag.clone())));

    if skip || !include {
        PostprocessorResult::StopAndSkipNote
    } else {
        PostprocessorResult::Continue
    }
}

#[test]
fn test_filter_tags() {
    let tags = vec![
        Value::String("skip".into()),
        Value::String("publish".into()),
    ];
    let empty_tags = vec![];
    assert_eq!(
        filter_by_tags_(&empty_tags, &[], &[]),
        PostprocessorResult::Continue,
        "When no exclusion & inclusion are specified, files without tags are included"
    );
    assert_eq!(
        filter_by_tags_(&tags, &[], &[]),
        PostprocessorResult::Continue,
        "When no exclusion & inclusion are specified, files with tags are included"
    );
    assert_eq!(
        filter_by_tags_(&tags, &["exclude".into()], &[]),
        PostprocessorResult::Continue,
        "When exclusion tags don't match files with tags are included"
    );
    assert_eq!(
        filter_by_tags_(&empty_tags, &["exclude".into()], &[]),
        PostprocessorResult::Continue,
        "When exclusion tags don't match files without tags are included"
    );
    assert_eq!(
        filter_by_tags_(&tags, &[], &["publish".into()]),
        PostprocessorResult::Continue,
        "When exclusion tags don't match files with tags are included"
    );
    assert_eq!(
        filter_by_tags_(&empty_tags, &[], &["include".into()]),
        PostprocessorResult::StopAndSkipNote,
        "When inclusion tags are specified files without tags are excluded"
    );
    assert_eq!(
        filter_by_tags_(&tags, &[], &["include".into()]),
        PostprocessorResult::StopAndSkipNote,
        "When exclusion tags don't match files with tags are exluded"
    );
    assert_eq!(
        filter_by_tags_(&tags, &["skip".into()], &["skip".into()]),
        PostprocessorResult::StopAndSkipNote,
        "When both inclusion and exclusion tags are the same exclusion wins"
    );
    assert_eq!(
        filter_by_tags_(&tags, &["skip".into()], &["publish".into()]),
        PostprocessorResult::StopAndSkipNote,
        "When both inclusion and exclusion tags match exclusion wins"
    );
}
