//! Recognition and rewriting of Obsidian `%%...%%` comments.
//!
//! Obsidian supports comments by wrapping text with `%%`; they are visible
//! only in editing views and never rendered. This module recognizes comment
//! spans in an expanded event stream (wikilinks resolved, embeds spliced in)
//! and rewrites them according to [`CommentsMode`].
//!
//! Recognition is conservative where Obsidian itself declines to interpret
//! the syntax: `%%` inside code blocks, inline code, math, tables, and
//! link/image labels never opens or closes a comment. Everywhere else a
//! non-greedy first-`%%`-to-next-`%%` pairing is used, mirroring Obsidian's
//! plain-text pairing — including comments that span blank lines and other
//! block containers. An unclosed `%%` is kept verbatim.
//!
//! Rewritten comments are emitted as raw HTML events because
//! [pulldown-cmark-to-cmark] escapes `<` inside `Event::Text`. A comment
//! confined to one paragraph becomes a single `Event::InlineHtml`; a comment
//! spanning block boundaries is emitted as an HTML block with the open
//! container skeleton closed before it and reopened after it, so the event
//! stream stays balanced.
//!
//! Known serialization trade-off: reopening a container below a block-level
//! comment restarts an interrupted ordered list at its start number
//! (`List(Some(1))` carries no "current index"), so `1. %%...\n2. %%...` tail
//! comment resume numbering from their start value. The event
//! model cannot express the continuation.
//!
//! [pulldown-cmark-to-cmark]: https://docs.rs/pulldown-cmark-to-cmark

use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

use super::MarkdownEvents;

/// What to do with Obsidian `%%...%%` comments during export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommentsMode {
    /// Leave `%%...%%` literals untouched. The default; matches the
    /// behavior of upstream obsidian-export.
    #[default]
    Keep,
    /// Rewrite comments to `<!-- ... -->` HTML comments that survive in the
    /// output source but are not rendered.
    Convert,
    /// Remove comments from the output entirely.
    Strip,
}

impl CommentsMode {
    /// Parse a mode name accepted by the CLI (`keep` / `convert` / `strip`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "keep" => Some(Self::Keep),
            "convert" => Some(Self::Convert),
            "strip" => Some(Self::Strip),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_name(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Convert => "convert",
            Self::Strip => "strip",
        }
    }
}

/// Container tags inside which `%%` is never treated as a comment marker,
/// matching where Obsidian itself does not interpret the syntax: code
/// blocks, tables (block level) and links, images (inline level).
const fn is_immune_start(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::CodeBlock(_) | Tag::Link { .. } | Tag::Image { .. } | Tag::Table(_)
    )
}

const fn is_immune_end(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::CodeBlock | TagEnd::Link | TagEnd::Image | TagEnd::Table
    )
}

/// Block-level container tags tracked for the skeleton replay: everything
/// that can hold paragraph-like content. Immune block containers (code
/// blocks, tables) are excluded — a comment cannot open inside them, so
/// they never appear on the open stack.
const fn is_block_start(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::TableHead
            | Tag::TableRow
            | Tag::MetadataBlock(_)
    )
}

const fn is_block_end(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::MetadataBlock(_)
    )
}

/// Whether the comment spanning `[start_idx..=end_idx]` crosses out of its
/// inline context. Immune *block* containers (code blocks, tables) count as
/// block boundaries here — a comment that swallowed one is necessarily
/// block-level — while links and images (inline) do not.
// Byte arithmetic and slicing below are safe: every offset comes from a
// prior `str::find` result or a fixed `%%` (ASCII, 2 bytes) step, so indices
// always land on char boundaries and stay within the text length. The
// same allowance pattern is used by the reference state machine in lib.rs.
// match_same_arms: the `return true` arms carry different guards (immune
// block containers vs. plain block boundaries) and cannot be merged.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::match_same_arms
)]
fn spans_block_boundary(events: &[Event<'_>], start_idx: usize, end_idx: usize) -> bool {
    let mut immune = 0_usize;
    for event in &events[start_idx..=end_idx] {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::Table(_))
            | Event::End(TagEnd::CodeBlock | TagEnd::Table) => return true,
            Event::Start(tag) if is_immune_start(tag) => immune += 1,
            Event::End(end) if is_immune_end(*end) => immune = immune.saturating_sub(1),
            Event::Start(tag) if immune == 0 && is_block_start(tag) => return true,
            Event::End(end) if immune == 0 && is_block_end(*end) => return true,
            Event::Html(_) | Event::DisplayMath(_) | Event::Rule => return true,
            _ => {}
        }
    }
    false
}

/// Find the closing `%%` at or after byte offset `from` of `events[start_idx]`,
/// continuing through subsequent events. Text inside immune containers is
/// skipped: a `%%` there is content, not a closing marker.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::string_slice
)]
fn find_close(events: &[Event<'_>], start_idx: usize, from: usize) -> Option<(usize, usize)> {
    let mut immune = 0_usize;
    for (i, event) in events.iter().enumerate().skip(start_idx) {
        match event {
            Event::Text(text) if immune == 0 => {
                let search_from = if i == start_idx { from } else { 0 };
                if let Some(pos) = text[search_from..].find("%%") {
                    return Some((i, search_from + pos));
                }
            }
            Event::Start(tag) if is_immune_start(tag) => immune += 1,
            Event::End(end) if is_immune_end(*end) => immune = immune.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Reconstruct a near-verbatim text for the comment content between the
/// opening marker at `(start_idx, content_from)` and the closing marker at
/// `(close_idx, close_to)`. Inline formatting events are synthesized back
/// to their punctuation (the same trick `expand_references` uses); block
/// boundaries become newlines. The result is only ever re-emitted as part
/// of an HTML comment, so approximate literals are acceptable.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::string_slice
)]
fn synthesize(
    events: &[Event<'_>],
    start_idx: usize,
    content_from: usize,
    close_idx: usize,
    close_to: usize,
) -> String {
    let mut buf = String::new();
    for (i, event) in events
        .iter()
        .enumerate()
        .take(close_idx + 1)
        .skip(start_idx)
    {
        match event {
            Event::Text(text) => {
                let slice: &str = if i == start_idx && i == close_idx {
                    &text[content_from..close_to]
                } else if i == start_idx {
                    &text[content_from..]
                } else if i == close_idx {
                    &text[..close_to]
                } else {
                    text
                };
                buf.push_str(slice);
            }
            Event::Code(code) => {
                buf.push('`');
                buf.push_str(code);
                buf.push('`');
            }
            Event::InlineMath(math) => {
                buf.push('$');
                buf.push_str(math);
                buf.push('$');
            }
            Event::DisplayMath(math) => {
                buf.push_str("$$");
                buf.push_str(math);
                buf.push_str("$$");
            }
            Event::InlineHtml(html) | Event::Html(html) => buf.push_str(html),
            Event::SoftBreak | Event::HardBreak => buf.push('\n'),
            Event::Rule => buf.push_str("\n---\n"),
            Event::FootnoteReference(label) => {
                buf.push_str("[^");
                buf.push_str(label);
                buf.push(']');
            }
            Event::TaskListMarker(checked) => {
                buf.push_str(if *checked { "[x] " } else { "[ ] " });
            }
            Event::Start(_) | Event::End(_) => synthesize_tag(event, &mut buf),
        }
    }
    buf
}

/// Tag half of [`synthesize`]: approximate the literal punctuation of a
/// `Start`/`End` event inside comment content.
fn synthesize_tag(event: &Event<'_>, buf: &mut String) {
    match event {
        Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => buf.push('*'),
        Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => buf.push_str("**"),
        Event::Start(Tag::Strikethrough) | Event::End(TagEnd::Strikethrough) => buf.push_str("~~"),
        Event::Start(Tag::Superscript) | Event::End(TagEnd::Superscript) => buf.push('^'),
        Event::Start(Tag::Subscript) | Event::End(TagEnd::Subscript) => buf.push('~'),
        // Links and images are immune to comment recognition but can still
        // end up *inside* a comment; keep their text readable and drop the
        // destinations (the End event carries no URL data anyway).
        Event::Start(Tag::Link { .. } | Tag::Image { .. }) => buf.push('['),
        Event::End(TagEnd::Link | TagEnd::Image) => buf.push(']'),
        Event::End(TagEnd::Paragraph) => buf.push_str("\n\n"),
        // Heading markers, list bullets and quote markers inside a comment
        // are approximated so the content stays readable as plain text.
        Event::Start(Tag::Heading { level, .. }) => {
            buf.push_str(match level {
                HeadingLevel::H1 => "# ",
                HeadingLevel::H2 => "## ",
                HeadingLevel::H3 => "### ",
                HeadingLevel::H4 => "#### ",
                HeadingLevel::H5 => "##### ",
                HeadingLevel::H6 => "###### ",
            });
        }
        Event::Start(Tag::BlockQuote(_)) => buf.push_str("> "),
        Event::Start(Tag::Item) => buf.push_str("\n- "),
        Event::Start(Tag::CodeBlock(kind)) => {
            buf.push_str("\n```");
            if let pulldown_cmark::CodeBlockKind::Fenced(info) = kind {
                buf.push_str(info);
            }
            buf.push('\n');
        }
        Event::End(TagEnd::CodeBlock) => buf.push_str("\n```\n"),
        Event::Start(Tag::TableCell) => buf.push(' '),
        // Every remaining block boundary (heading/quote ends, list item,
        // list and footnote ends, table row ends) separates words in the
        // synthesized content; without these newlines adjacent words would
        // fuse (`x` + `para` -> `xpara`).
        Event::End(
            TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::Item
            | TagEnd::List(_)
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::TableHead
            | TagEnd::TableRow,
        ) => buf.push('\n'),
        _ => {}
    }
}

/// Make comment content safe to embed between `<!--` and `-->` for
/// CommonMark-based renderers: `--` sequences would terminate the comment
/// early (or make it invalid inline HTML), and a body starting with `>` or
/// `->` is illegal in inline comments.
///
/// The fold repeats until no `--` remains: a single non-overlapping pass
/// lets a run like `--->` recombine into `-->` across the inserted space
/// (`- -` + `->`), resurrecting the very terminator this guards against.
fn sanitize(content: &str) -> String {
    let trimmed = content.trim();
    let mut out = trimmed.to_owned();
    while out.contains("--") {
        out = out.replace("--", "- -");
    }
    if out.starts_with('>') {
        out.insert(0, ' ');
    }
    if out.ends_with('-') {
        out.push(' ');
    }
    out
}

fn owned(text: String) -> CowStr<'static> {
    CowStr::Boxed(text.into_boxed_str())
}

/// The set of container `End` events occurring in the comment region
/// `[start_idx..=close_idx]`, immune regions included (immune containers
/// never appear on the skeleton stack, so their Ends never match a lookup).
#[allow(clippy::indexing_slicing)]
fn region_end_set(
    source: &[Event<'_>],
    start_idx: usize,
    close_idx: usize,
) -> std::collections::HashSet<TagEnd> {
    source[start_idx..=close_idx]
        .iter()
        .filter_map(|event| match event {
            Event::End(end) => Some(*end),
            _ => None,
        })
        .collect()
}

/// Net surplus of inline formatting containers per tag type opened
/// (positive) or closed (negative) inside the comment region. Emphasis and
/// friends can straddle a comment boundary; the half inside the region is
/// synthesized into the comment body, leaving the outer half unpaired.
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn inline_surplus(source: &[Event<'_>], start_idx: usize, close_idx: usize) -> Vec<(TagEnd, i64)> {
    let mut immune = 0_usize;
    let mut counts: std::collections::BTreeMap<TagEnd, i64> = std::collections::BTreeMap::new();
    for event in &source[start_idx..=close_idx] {
        match event {
            Event::Start(tag) if is_immune_start(tag) => immune += 1,
            Event::End(end) if is_immune_end(*end) => immune = immune.saturating_sub(1),
            Event::Start(tag) if immune == 0 && is_inline_format_start(tag) => {
                *counts.entry(tag.to_end()).or_default() += 1;
            }
            Event::End(end) if immune == 0 && is_inline_format_end(*end) => {
                *counts.entry(*end).or_default() -= 1;
            }
            _ => {}
        }
    }
    counts.into_iter().filter(|(_, n)| *n != 0).collect()
}

const fn is_inline_format_start(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Superscript | Tag::Subscript
    )
}

const fn is_inline_format_end(end: TagEnd) -> bool {
    matches!(
        end,
        TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Superscript
            | TagEnd::Subscript
    )
}

/// Re-emit the opening halves of inline formatting containers the comment
/// cut through (positive surplus), so the Ends waiting outside the region
/// stay paired.
fn reopen_inline_surplus(surplus: &[(TagEnd, i64)], out: &mut MarkdownEvents<'_>) {
    for (end, count) in surplus {
        let Some(tag) = inline_start_of(*end) else {
            continue;
        };
        for _ in 0..*count {
            out.push(Event::Start(tag.clone()));
        }
    }
}

const fn inline_start_of(end: TagEnd) -> Option<Tag<'static>> {
    match end {
        TagEnd::Emphasis => Some(Tag::Emphasis),
        TagEnd::Strong => Some(Tag::Strong),
        TagEnd::Strikethrough => Some(Tag::Strikethrough),
        TagEnd::Superscript => Some(Tag::Superscript),
        TagEnd::Subscript => Some(Tag::Subscript),
        _ => None,
    }
}

/// Append a text run, dropping leading whitespace when it would land right
/// after a container start. The serializer escapes such whitespace
/// (`&#32;`), and it only ever appears here as the tail of a text event
/// that got split by comment rewriting — parsed content never starts an
/// inline run with meaningful leading spaces.
fn push_text(out: &mut MarkdownEvents<'_>, text: &str) {
    let trimmed: &str = if matches!(out.last(), Some(Event::Start(_))) {
        text.trim_start_matches([' ', '\t'])
    } else {
        text
    };
    if !trimmed.is_empty() {
        out.push(Event::Text(owned(trimmed.to_owned())));
    }
}

/// Rewrite every recognized comment in `events` according to `mode`.
///
/// The pass rebuilds the event stream: structural edits (dropping events,
/// closing and reopening containers around block-level comments) cannot be
/// done in place. `text_pos` tracks how much of the current `Event::Text`
/// has already been consumed, so text after a closing `%%` is rescanned for
/// further comments.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::string_slice
)]
pub fn rewrite_events(events: &mut MarkdownEvents<'_>, mode: CommentsMode) {
    if mode == CommentsMode::Keep {
        return;
    }

    let source = std::mem::take(events);
    let mut out: MarkdownEvents<'_> = Vec::with_capacity(source.len());
    // Open block-level containers (the skeleton stack), maintained by the
    // main loop and handed to `emit_comment` for block-level comments.
    let mut stack: Vec<Tag<'_>> = Vec::new();
    let mut immune = 0_usize;
    let mut idx = 0_usize;
    let mut text_pos = 0_usize;

    while idx < source.len() {
        let event = &source[idx];
        match event {
            Event::Text(text) if immune == 0 => {
                if text_pos >= text.len() {
                    // Nothing left of this text event (e.g. the tail right
                    // after a comment that just closed).
                    idx += 1;
                    text_pos = 0;
                    continue;
                }
                match text[text_pos..].find("%%") {
                    None => {
                        push_text(&mut out, &text[text_pos..]);
                        idx += 1;
                        text_pos = 0;
                    }
                    Some(pos) => {
                        let open_at = text_pos + pos;
                        if open_at > text_pos {
                            out.push(Event::Text(owned(text[text_pos..open_at].to_string())));
                        }
                        match find_close(&source, idx, open_at + 2) {
                            None => {
                                // Unclosed: replay verbatim from the opening
                                // marker to the end of the stream.
                                out.push(Event::Text(owned(text[open_at..].to_string())));
                                out.extend(source[idx + 1..].iter().cloned());
                                idx = source.len();
                            }
                            Some((close_idx, close_at)) => {
                                // Emit the comment; the returned index is
                                // where the main loop resumes (the closing
                                // event itself when trailing text remains,
                                // past it when trailing Ends were elided).
                                let next = emit_comment(
                                    &source,
                                    idx,
                                    open_at + 2,
                                    close_idx,
                                    close_at,
                                    mode,
                                    &mut out,
                                    &mut stack,
                                );
                                idx = next;
                                text_pos = if next == close_idx { close_at + 2 } else { 0 };
                            }
                        }
                    }
                }
            }
            Event::Start(tag) if is_immune_start(tag) => {
                immune += 1;
                out.push(event.clone());
                idx += 1;
            }
            Event::End(end) if is_immune_end(*end) => {
                immune = immune.saturating_sub(1);
                out.push(event.clone());
                idx += 1;
            }
            Event::Start(tag) if immune == 0 && is_block_start(tag) => {
                stack.push(tag.clone());
                out.push(event.clone());
                idx += 1;
            }
            Event::End(end) if immune == 0 && is_block_end(*end) => {
                // A valid input stream (and a consistent skeleton stack)
                // always pairs this End with an earlier Start; the assert
                // turns a silent desync into a loud failure in debug runs.
                let popped = stack.pop();
                debug_assert!(
                    popped.is_some(),
                    "block End without matching open container"
                );
                out.push(event.clone());
                idx += 1;
            }
            _ => {
                out.push(event.clone());
                idx += 1;
            }
        }
    }

    *events = out;
}

/// Emit the comment spanning `start_idx..=close_idx` (content between byte
/// `content_from` of the opening event and byte `close_to` of the closing
/// event) into `out`, and update `stack` to the containers open at the
/// point where the main loop resumes.
///
/// Returns the event index the main loop should continue from: the closing
/// event itself when text remains after it, or past any trailing container
/// `End` events consumed by the elision below.
///
/// Inline comments are replaced in place by one `Event::InlineHtml`. Block
/// comments close the open-container skeleton before them and reopen it
/// after. Containers the comment would orphan (e.g. a paragraph holding
/// only the opening `%%`) are elided on both sides so the output carries no
/// stray empty blocks.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::too_many_arguments,
    clippy::cognitive_complexity,
    clippy::too_many_lines
)]
fn emit_comment<'a>(
    source: &[Event<'a>],
    start_idx: usize,
    content_from: usize,
    close_idx: usize,
    close_to: usize,
    mode: CommentsMode,
    out: &mut MarkdownEvents<'a>,
    stack: &mut Vec<Tag<'a>>,
) -> usize {
    let block = spans_block_boundary(source, start_idx, close_idx);
    let content = sanitize(&synthesize(
        source,
        start_idx,
        content_from,
        close_idx,
        close_to,
    ));
    let rest: &str = match &source[close_idx] {
        Event::Text(text) => &text[close_to + 2..],
        // find_close only ever matches Text events; the fallback keeps a
        // future change from turning into a slicing panic.
        _ => {
            debug_assert!(false, "close_idx must point at a Text event");
            ""
        }
    };
    // Inline formatting halves cut by the comment: an emphasis opened in
    // the region and closed outside it leaves a dangling outer End (and
    // vice versa). The surplus is re-emitted around the comment so the
    // stream stays balanced even though the region's half was synthesized
    // into the comment body.
    let surplus = inline_surplus(source, start_idx, close_idx);

    if !block {
        if mode == CommentsMode::Strip && rest.trim().is_empty() {
            // The comment was the only thing left in its containers (e.g. a
            // one-line note): elide them in matching pairs — a container is
            // only dropped when both its Start (tail of `out`) and its End
            // (next event) are present — rather than leaving empty shells.
            // The region crossed no block boundary, so the stack is already
            // the closing-point state.
            let mut next = close_idx + 1;
            while let Some(tag) = stack.last().cloned() {
                if out.last() == Some(&Event::Start(tag.clone()))
                    && source.get(next) == Some(&Event::End(tag.to_end()))
                {
                    out.pop();
                    stack.pop();
                    next = next.saturating_add(1);
                } else {
                    break;
                }
            }
            return next;
        }
        for (end, deficit) in &surplus {
            for _ in 0..-*deficit {
                out.push(Event::End(*end));
            }
        }
        if mode == CommentsMode::Convert {
            let html = if content.is_empty() {
                "<!---->".to_owned()
            } else {
                format!("<!-- {content} -->")
            };
            out.push(Event::InlineHtml(owned(html)));
        }
        reopen_inline_surplus(&surplus, out);
        // Trailing text is left to the main loop (text_pos), which rescans
        // it for further comments.
        return close_idx;
    }

    // Elide empty containers directly above the comment: if `out` ends with
    // exactly the Start of the innermost open container, both the Start and
    // the stack entry are dropped so no dangling pair remains. A container
    // only qualifies when the comment region actually contains its End —
    // that End is what the replay below consumes. Without this check, a
    // comment that starts in a container's first paragraph and closes in a
    // later paragraph of the *same* container would drop the container even
    // though it was never left, and the reopened skeleton would be missing
    // a level.
    let stack_before_elision = stack.clone();
    let region_ends = region_end_set(source, start_idx, close_idx);
    while let Some(tag) = stack.last() {
        if out.last() == Some(&Event::Start(tag.clone())) && region_ends.contains(&tag.to_end()) {
            out.pop();
            stack.pop();
        } else {
            break;
        }
    }

    // Close whatever is still open above the comment, plus the inline
    // formatting halves closed outside the region.
    for (end, deficit) in &surplus {
        for _ in 0..-*deficit {
            out.push(Event::End(*end));
        }
    }
    for tag in stack.iter().rev() {
        out.push(Event::End(tag.to_end()));
    }

    if mode == CommentsMode::Convert {
        let html = if content.is_empty() {
            "<!---->\n".to_owned()
        } else {
            format!("<!--\n{content}\n-->\n")
        };
        out.push(Event::Start(Tag::HtmlBlock));
        out.push(Event::Html(owned(html)));
        out.push(Event::End(TagEnd::HtmlBlock));
    }

    // Advance a copy of the skeleton to the containers open at the closing
    // point by replaying the block-level boundaries swallowed by the
    // comment region. The replay starts from the *pre-elision* stack: an
    // elided container's End occurs inside the region and must consume that
    // very container, not whatever sits innermost on the elided stack.
    let mut closing_stack = stack_before_elision;
    let mut immune = 0_usize;
    for event in &source[start_idx..=close_idx] {
        match event {
            Event::Start(tag) if is_immune_start(tag) => immune += 1,
            Event::End(end) if is_immune_end(*end) => immune = immune.saturating_sub(1),
            Event::Start(tag) if immune == 0 && is_block_start(tag) => {
                closing_stack.push(tag.clone());
            }
            Event::End(end) if immune == 0 && is_block_end(*end) => {
                closing_stack.pop();
            }
            _ => {}
        }
    }

    if !rest.trim().is_empty() {
        // Text follows the closing marker: reopen the skeleton, and let the
        // main loop emit (and rescan) the trailing text.
        for tag in &closing_stack {
            out.push(Event::Start(tag.clone()));
        }
        reopen_inline_surplus(&surplus, out);
        *stack = closing_stack;
        return close_idx;
    }

    // No trailing text: elide empty containers directly below the comment.
    // If the events right after the closing marker are exactly the End
    // events of the innermost open containers, consume them and emit the
    // matching Starts only for whatever remains open.
    let next = elide_trailing_empty(source, close_idx + 1, &mut closing_stack);
    let reopened = !closing_stack.is_empty();
    for tag in &closing_stack {
        out.push(Event::Start(tag.clone()));
    }
    if reopened {
        reopen_inline_surplus(&surplus, out);
        // A soft/hard break directly after a freshly reopened container is
        // a residue of the comment's own line ending, not content: passing
        // it through would start the reopened item with a stray newline.
        if matches!(source.get(next), Some(Event::SoftBreak | Event::HardBreak)) {
            *stack = closing_stack;
            return next.saturating_add(1);
        }
    }
    *stack = closing_stack;
    next
}

/// Consume container `End` events at `next` that close the innermost open
/// containers; those Starts are dropped instead of being re-emitted, so no
/// empty pair survives below a fully-consumed comment. Returns the index of
/// the first event left unconsumed and trims `stack` accordingly.
fn elide_trailing_empty(source: &[Event<'_>], next: usize, stack: &mut Vec<Tag<'_>>) -> usize {
    let mut next = next;
    while let Some(tag) = stack.last() {
        if source.get(next) == Some(&Event::End(tag.to_end())) {
            stack.pop();
            next = next.saturating_add(1);
        } else {
            break;
        }
    }
    next
}

#[cfg(test)]
mod tests {
    #![allow(clippy::shadow_unrelated)]

    use super::*;

    fn run(markdown: &str, mode: CommentsMode) -> String {
        let mut events: MarkdownEvents<'_> =
            pulldown_cmark::Parser::new_ext(markdown, crate::markdown_parser_options())
                .into_offset_iter()
                .map(|(event, _)| event)
                .collect();
        rewrite_events(&mut events, mode);
        // The rewritten stream must stay balanced by itself: the serializer
        // (and re-parsing) would silently paper over structural damage. The
        // check is stack-based, not a running sum: a dangling End paired
        // with an unclosed Start would cancel out and slip through a sum.
        let mut open: Vec<TagEnd> = Vec::new();
        for event in &events {
            match event {
                Event::Start(tag) => open.push(tag.to_end()),
                Event::End(end) => {
                    assert_eq!(open.pop(), Some(*end), "unbalanced event stream");
                }
                _ => {}
            }
        }
        assert!(open.is_empty(), "unclosed containers remain");
        crate::render_mdevents_to_mdtext(&events)
    }

    fn convert(markdown: &str) -> String {
        run(markdown, CommentsMode::Convert)
    }

    fn strip(markdown: &str) -> String {
        run(markdown, CommentsMode::Strip)
    }

    #[test]
    fn inline_comment_becomes_html_comment() {
        assert_eq!(
            convert("before %%secret%% after\n"),
            "before <!-- secret --> after\n"
        );
    }

    #[test]
    fn multiple_inline_comments() {
        assert_eq!(
            convert("%%one%% mid %%two%% end\n"),
            "<!-- one --> mid <!-- two --> end\n"
        );
    }

    #[test]
    fn unclosed_marker_stays_verbatim() {
        assert_eq!(
            convert("text %% never closed\nmore text\n"),
            "text %% never closed\nmore text\n"
        );
    }

    #[test]
    fn code_blocks_are_immune() {
        // The serializer re-picks fence lengths on round-trips; what
        // matters is that the `%%` literals survive unconverted.
        let output = convert("text\n\n```md\n%%inside code%%\n```\n\nmore\n");
        assert!(output.contains("%%inside code%%"), "check failed");
        assert!(!output.contains("<!--"), "check failed");
    }

    #[test]
    fn inline_code_is_immune() {
        assert_eq!(
            convert("run `%%literal%%` now\n"),
            "run `%%literal%%` now\n"
        );
    }

    #[test]
    fn math_is_immune() {
        let input = "math $x %%not a comment%% y$ inline\n";
        assert_eq!(convert(input), input);
    }

    #[test]
    fn tables_are_immune() {
        // The serializer re-flows table pipes on any round-trip; what
        // matters here is that the `%%` literals survive unconverted.
        let output = convert("| a %% b |\n| --- |\n| c %% d |\n");
        assert_eq!(output.matches("%%").count(), 2, "check failed");
        assert!(!output.contains("<!--"), "check failed");
    }

    #[test]
    fn link_text_is_immune() {
        let input = "[label %% stays](note.md) and ![alt %% stays](img.png)\n";
        assert_eq!(convert(input), input);
    }

    #[test]
    fn single_paragraph_multiline_comment_stays_inline() {
        // No blank line: one paragraph joined by soft breaks, so this takes
        // the inline path (verified via spans_block_boundary).
        assert_eq!(convert("%%\nnote to self\n%%\n"), "<!-- note to self -->\n");
    }

    #[test]
    fn comment_spanning_blank_lines_is_block_level() {
        assert_eq!(
            convert("%%\nfirst para\n\nsecond para\n%%\n"),
            "<!--\nfirst para\n\nsecond para\n-->\n\n"
        );
    }

    #[test]
    fn formatting_inside_comment_is_synthesized() {
        assert_eq!(convert("%%a *b* c%%\n"), "<!-- a *b* c -->\n");
    }

    #[test]
    fn double_dashes_are_neutralized() {
        assert_eq!(convert("%%x --> y -- z%%\n"), "<!-- x - -> y - - z -->\n");
    }

    #[test]
    fn triple_dash_arrow_is_not_resurrected() {
        // A single non-overlapping `--` fold would turn `--->` into
        // `- -` + `->`, recombining into `-->` across the inserted space.
        for input in ["a %%x ---> y%% b\n", "%% ---> %%\n", "%%x---->y%%\n"] {
            let output = convert(input);
            // Exactly one terminator: the closing marker. A resurrected one
            // would appear before it and leak the tail as visible text.
            assert_eq!(output.matches("-->").count(), 1, "terminator count wrong");
            let reparsed: Vec<Event<'_>> =
                pulldown_cmark::Parser::new_ext(&output, crate::markdown_parser_options())
                    .collect();
            let html_count = reparsed
                .iter()
                .filter(|e| matches!(e, Event::Html(_) | Event::InlineHtml(_)))
                .count();
            assert_eq!(html_count, 1, "expected exactly one HTML event");
        }
    }

    #[test]
    fn strip_mode_removes_comments() {
        // Two spaces: "before " prefix plus the " after" tail around the
        // removed marker — intentional, not a typo.
        assert_eq!(strip("before %%secret%% after\n"), "before  after\n");
        // The serializer appends a trailing newline even for an empty
        // stream, so a fully-stripped note serializes to a single "\n".
        assert_eq!(strip("%%\nwhole block\n%%\n"), "\n");
    }

    #[test]
    fn keep_mode_is_identity() {
        let input = "a %%comment%% b\n";
        assert_eq!(run(input, CommentsMode::Keep), input);
    }

    #[test]
    fn comment_inside_list_item() {
        // The serializer emits `*` bullets regardless of the input marker.
        assert_eq!(
            convert("- item %%note%% here\n"),
            "* item <!-- note --> here\n"
        );
    }

    #[test]
    fn comment_spanning_list_items_keeps_balance() {
        let output = convert("- one %%starts\n- two %%ends\n- three\n");
        assert!(output.contains("<!--"));
        assert!(output.contains("-->"));
        assert!(output.contains("three"));
        assert_balanced(&output);
    }

    #[test]
    fn comment_with_surrounding_text_keeps_paragraphs() {
        let output = convert("before %%\nspanning\n\nparas\n%% after\n");
        assert!(output.starts_with("before"));
        assert!(output.contains("<!--\nspanning\n\nparas\n-->"));
        assert!(output.trim_end().ends_with("after"));
        assert_balanced(&output);
    }

    #[test]
    fn round_trip_output_reparses_as_html() {
        for output in [
            convert("%%\nmulti\n\nline\n%%\n"),
            convert("a %%inline%% b\n"),
        ] {
            let reparsed: Vec<Event<'_>> =
                pulldown_cmark::Parser::new_ext(&output, crate::markdown_parser_options())
                    .collect();
            assert!(
                reparsed
                    .iter()
                    .any(|e| matches!(e, Event::Html(_) | Event::InlineHtml(_))),
                "expected an HTML event in {:?}",
                output
            );
        }
    }

    #[test]
    fn code_block_inside_comment_does_not_close_it() {
        // A `%%` inside a code block swallowed by a comment is content, not
        // a closing marker. The trailing "after" sits before the closing
        // marker, so it belongs to the comment itself.
        let output = convert("%% before\n\n```md\n%% not a closer\n```\n\nafter %%\n");
        assert!(output.starts_with("<!--\nbefore"), "check failed");
        assert!(output.contains("%% not a closer"), "check failed");
        assert!(output.contains("-->"), "check failed");
        assert!(output.contains("after"), "check failed");
    }

    #[test]
    fn lone_percent_signs_are_ignored() {
        assert_eq!(convert("50% off, 100%% done\n"), "50% off, 100%% done\n");
    }

    #[test]
    fn pairing_is_non_greedy() {
        // The first `%%` pairs with the next one; a leftover marker in the
        // trailing text stays literal.
        assert_eq!(convert("%%a %% b%% c\n"), "<!-- a --> b%% c\n");
    }

    #[test]
    fn mode_names_round_trip() {
        for mode in [
            CommentsMode::Keep,
            CommentsMode::Convert,
            CommentsMode::Strip,
        ] {
            assert_eq!(CommentsMode::from_name(mode.as_name()), Some(mode));
        }
        assert_eq!(CommentsMode::from_name("bogus"), None);
    }

    #[test]
    fn comment_within_one_container_keeps_content_inside() {
        // A comment opening in a container's first paragraph and closing in
        // a later paragraph of the *same* container must not elide the
        // container: the trailing text stays inside it. (Regression: the
        // leading elision used to drop containers the comment never left,
        // letting the tail escape to the top level.)
        let output = convert("> %%x\n>\n> tail%% y\n");
        assert!(output.contains("<!--\nx\n\ntail\n-->"), "check failed");
        assert!(output.contains("> y"), "tail must stay inside the quote");

        let output = convert("- %%x\n\n  tail%% y\n");
        assert!(output.contains("<!--\nx\n\ntail\n-->"), "check failed");
        assert!(output.contains("* y"), "tail must stay inside the item");

        let output = convert("> > %%x\n> >\n> > tail%% y\n");
        assert!(output.contains("<!--\nx\n\ntail\n-->"), "check failed");

        for input in [
            "> %%x\n>\n> tail%% y\n",
            "- %%x\n\n  tail%% y\n",
            "> > %%x\n> >\n> > tail%% y\n",
        ] {
            // Strip must keep the same container containment.
            strip(input);
        }
    }

    #[test]
    fn emphasis_cut_by_comment_keeps_stream_balanced() {
        // The comment body synthesizes the inner `*` half; the outer half
        // waiting past the closing marker must be re-paired.
        let output = convert("a %%x *b%% c* d\n");
        assert!(output.contains("<!--"), "check failed");
        let output = convert("a *b %%x* c%% d\n");
        assert!(output.contains("<!--"), "check failed");
        // Strong and strikethrough behave the same.
        convert("a %%x **b%% c** d\n");
        convert("a %%x ~~b%% c~~ d\n");
        // Strip mode must also rebalance.
        strip("a %%x *b%% c* d\n");
        strip("a *b %%x* c%% d\n");
    }

    #[test]
    fn block_boundary_does_not_fuse_words() {
        // Item/list ends inside a comment region separate words with a
        // newline (regression: they used to be silently dropped, fusing
        // "x" and "para" into "xpara").
        let output = convert("- %%x\n\npara%% tail\n");
        assert!(output.contains("x\n\npara"), "check failed");
    }

    #[test]
    fn reopened_container_does_not_start_with_stray_break() {
        // The soft break right after the closing marker is the comment's
        // own line ending, not content of the reopened container.
        let output = convert("%%\n- a\n- b\n%%\ntail\n");
        assert!(!output.contains("\n  tail"), "stray item prefix leaked");
        assert!(output.contains("tail"), "check failed");
    }

    /// Re-parse serialized output and assert Start/End events pair up.
    fn assert_balanced(markdown: &str) {
        let events: Vec<Event<'_>> =
            pulldown_cmark::Parser::new_ext(markdown, crate::markdown_parser_options()).collect();
        let mut depth = 0_i64;
        for event in &events {
            match event {
                Event::Start(_) => depth = depth.saturating_add(1),
                Event::End(_) => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        assert_eq!(depth, 0, "re-parsed stream must be balanced");
    }
}
