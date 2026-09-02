# Advanced usage

## Frontmatter

By default, frontmatter is copied over "as-is".

Some static site generators are picky about frontmatter and require it to be present.
Some get tripped up when Markdown files don't have frontmatter but start with a list item or horizontal rule.
In these cases, `--frontmatter=always` can be used to insert an empty frontmatter entry.

To completely remove any frontmatter from exported notes, use `--frontmatter=never`.

## Missing sections

An embed pointing at a section (heading) that doesn't exist in the target note is handled according to `--missing-section`:

* `--missing-section skip` (the default): the embed is replaced with nothing and a warning is emitted. Closest to Obsidian's own "not found" rendering.
* `--missing-section embed-full`: the entire note is embedded (the historical behavior of this tool).
* `--missing-section fail`: the export of the note containing the embed fails with an error.

The strategy is applied independently at every level of embedding: a missing section only affects that single embed, never the rest of the parent note.

Block references (`![[note#^block-id]]`) locate the block the id marks (a paragraph, a list item, or a whole quote block; an id alone on its own line marks the block above it). The `--missing-section` strategy covers block references whose id doesn't exist in the target note. The id marker is stripped from the embedded copy — Obsidian doesn't display it — while id definitions in source notes are kept as-is.

Same-file section and block embeds (`![[#Heading]]` / `![[#^block-id]]`) are supported too, matching Obsidian's whole-file resolution: a reference inside a same-file embed first looks in the embedded slice, then falls back to the whole note, and only degrades per `--missing-section` when the section is nowhere in the file. One caveat remains: any same-file embed appearing inside an expansion of the same file degrades to a plain link (the check is file-level, so this includes same-file references to other sections that would be safe to expand).

## Obsidian comments

Obsidian comments (`%%like this%%`, including multi-line block comments) are only visible in Obsidian's editing views. By default they are kept verbatim, which renders as literal `%%` text in plain Markdown consumers. Use `--comments` to choose what happens to them:

* `--comments keep` (the default): comments stay as literal `%%...%%` text.
* `--comments convert`: each comment becomes an HTML comment (`<!-- ... -->`) that survives in the output source but is not rendered.
* `--comments strip`: comments are removed from the output entirely.

Recognition follows Obsidian's plain-text pairing: the first `%%` pairs with the next `%%`, even across blank lines and list or quote boundaries, and an unclosed `%%` stays literal. `%%` inside code blocks, inline code, math, tables and link labels is never treated as a comment marker — the same places Obsidian itself declines to interpret the syntax. Content that would break the HTML comment syntax is neutralized (`--` becomes `- -`).

A comment spanning block boundaries splits the surrounding structure at the comment (e.g. a list item ends, the HTML comment follows as its own block, the remaining list restarts below); comments wholly inside one paragraph are rewritten in place. An interrupted ordered list restarts at its start number — CommonMark's list syntax carries no "current index".

Note for `--render-diagrams` users: the tool-availability pre-scan mirrors the `--comments` mode. A diagram code block sitting inside a `%%` comment counts (and requires its tool to be installed) only under `--comments keep`; with `strip` or `convert` the block never reaches the rendering stage, so it is not counted and its tool is not required.

## Failing files

By default, a note that fails to export (e.g. broken YAML frontmatter) is recorded and the export continues with the remaining notes; at the end, a summary listing every failing note is printed. Use `--fail-fast` to instead stop on the first failing file. Note that with parallel exports, files already being processed when the failure occurs may still complete.

## Diagram rendering

Obsidian renders special fenced code blocks (` ```dot `, ` ```mermaid `, …) through plugins, but plain Markdown consumers show them as literal code. With `--render-diagrams`, such blocks are rendered into standalone image files by shelling out to the corresponding local tools, and the export embeds a regular Markdown image reference instead:

```sh
obsidian-export --render-diagrams dot,mermaid,wavedrom,tikz SOURCE TARGET
```

Renderers and the external tools they require:

| Renderer | Code block languages | Requires | Formats |
|----------|----------------------|----------|---------|
| dot | `dot`, `graphviz` | [Graphviz](https://graphviz.org/download/) (`dot`) | svg, png |
| mermaid | `mermaid`, `mmd` | [mermaid-cli](https://github.com/mermaid-js/mermaid-cli) (`mmdc`) | svg, png |
| wavedrom | `wavedrom` | [wavedrom](https://www.npmjs.com/package/wavedrom) | svg |
| tikz | `tikz` | a TeX distribution with `latex` and `dvisvgm` (e.g. TeX Live) | svg |

Behavior details:

* **Tool discovery** prefers an explicit path (`--diagram-bin dot=/path/to/dot`, repeatable) and otherwise scans `PATH`. On Windows the scan honors `PATHEXT` and runs npm's `.cmd` shims through `cmd.exe`, so global npm installs work out of the box. (The `cmd.exe` wrapper expands `%VARIABLE%`-shaped substrings even inside quotes: on the rare path containing a paired `%`, point `--diagram-bin` at the underlying `.exe` to bypass the wrapper.)
* `--diagram-format` and `--diagram-bin` only take effect together with `--render-diagrams`; passing them alone draws a warning on stderr and is otherwise ignored.
* **Atomicity**: tools are resolved in a prescan, for the languages of the blocks that will actually render (see the `--comments` note above: under `strip`/`convert`, blocks inside `%%` comments do not require tools), *before* any output file is written. A missing tool aborts the export with exit code 1 and an install hint, leaving the destination untouched.
* **Per-block failures are non-fatal**: a diagram whose code the tool rejects stays a code block and produces a warning; the export always completes.
* **Output format**: `--diagram-format png` requests raster output; renderers without it (wavedrom, tikz) fall back to svg with a warning.
* **Assets** are written next to each note under `assets/<note>-<hash>.<ext>` (content-addressed over renderer + language + source + format), so unchanged blocks resolve to the same file across runs and re-exports skip the external tool entirely.
* **tikz** block content is the *inside* of a `tikzpicture` environment (Obsidian plugin convention); a source carrying its own `\begin{tikzpicture}` is embedded verbatim. Fonts are converted to paths (`dvisvgm --no-fonts`) for renderer compatibility; CJK text inside tikz drawings may render poorly — prefer mermaid or dot for those.

## Progress events

Passing `--progress json` emits machine-readable progress events on stdout as JSON Lines, one JSON object per line. This is intended for programs driving obsidian-export as a child process: the first line declares the schema version, followed by per-file progress, warnings, and a final end event. Without this flag, stdout stays silent.

## Ignoring files

The following files are not exported by default:

* hidden files (can be adjusted with `--hidden`)
* files matching a pattern listed in `.export-ignore` (can be adjusted with `--ignore-file`)
* any files that are ignored by git (can be adjusted with `--no-git`)
* using `--skip-tags foo --skip-tags bar` will skip any files that have the tags `foo` or `bar` in their frontmatter
* using `--only-tags foo --only-tags bar` will skip any files that **don't** have the tags `foo` or `bar` in their frontmatter

(See `--help` for more information).

Notes linking to ignored notes will be unlinked (they'll only include the link text).
Embeds of ignored notes will be skipped entirely.

### Ignorefile syntax

The syntax for `.export-ignore` files is identical to that of [gitignore] files.
Here's an example:

```
# Ignore the directory private that is located at the top of the export tree
/private
# Ignore any file or directory called `test`
test
# Ignore any PDF file
*.pdf
# ..but include special.pdf
!special.pdf
```

For more comprehensive documentation and examples, see the [gitignore] manpage.

## Recursive embeds

It's possible to end up with "recursive embeds" when two notes embed each other.
This happens for example when a `Note A.md` contains `![[Note B]]` but `Note B.md` also contains `![[Note A]]`.

By default, this will trigger an error and display the chain of notes which caused the recursion.

This behavior may be changed by specifying `--no-recursive-embeds`.
Using this mode, if a note is encountered for a second time while processing the original note, instead of embedding it again a link to the note is inserted instead to break the cycle.

## Relative links with Hugo

The [Hugo] static site generator [does not support relative links to files][hugo-relative-linking].
Instead, it expects you to link to other pages using the [`ref` and `relref` shortcodes].

As a result of this, notes that have been exported from Obsidian using obsidian-export do not work out of the box because Hugo doesn't resolve these links correctly.

[Markdown Render Hooks] (only supported using the default `goldmark` renderer) allow you to work around this issue however, making exported notes work with Hugo after a bit of one-time setup work.

Create the file `layouts/_default/_markup/render-link.html` with the following contents:

```
{{- $url := urls.Parse .Destination -}}
{{- $scheme := $url.Scheme -}}

<a href="
  {{- if eq $scheme "" -}}
    {{- if strings.HasSuffix $url.Path ".md" -}}
      {{- relref .Page .Destination | safeURL -}}
    {{- else -}}
      {{- .Destination | safeURL -}}
    {{- end -}}
  {{- else -}}
    {{- .Destination | safeURL -}}
  {{- end -}}"
  {{- with .Title }} title="{{ . | safeHTML }}"{{- end -}}>
  {{- .Text | safeHTML -}}
</a>

{{- /* whitespace stripped here to avoid trailing newline in rendered result caused by file EOL */ -}}
```

And `layouts/_default/_markup/render-image.html` for images:

```
{{- $url := urls.Parse .Destination -}}
{{- $scheme := $url.Scheme -}}

<img src="
  {{- if eq $scheme "" -}}
    {{- if strings.HasSuffix $url.Path ".md" -}}
      {{- relref .Page .Destination | safeURL -}}
    {{- else -}}
      {{- printf "/%s%s" .Page.File.Dir .Destination | safeURL -}}
    {{- end -}}
  {{- else -}}
    {{- .Destination | safeURL -}}
  {{- end -}}"
  {{- with .Title }} title="{{ . | safeHTML }}"{{- end -}}
  {{- with .Text }} alt="{{ . | safeHTML }}"
  {{- end -}}
/>

{{- /* whitespace stripped here to avoid trailing newline in rendered result caused by file EOL */ -}}
```

With these hooks in place, links to both notes as well as file attachments should now work correctly.

> Note: If you're using a theme which comes with it's own render hooks, you might need to do a little extra work, or customize the snippets above, to avoid conflicts with the hooks from your theme.

[`ref` and `relref` shortcodes]: https://gohugo.io/content-management/cross-references/
[gitignore]: https://git-scm.com/docs/gitignore
[hugo-relative-linking]: https://notes.nick.groenen.me/notes/relative-linking-in-hugo/
[hugo]: https://gohugo.io
[markdown render hooks]: https://gohugo.io/getting-started/configuration-markup#markdown-render-hooks
