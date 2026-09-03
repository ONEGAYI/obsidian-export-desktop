<!--

WARNING:

  Do not edit README.md directly, it is automatically generated from files in
  the docs directory.

  Instead of editing README.md, edit the corresponding Markdown files in the
  docs directory and run generate.sh.

  To add new sections, create new files under docs and add these to _combined.md

-->


# Obsidian Export

*Obsidian Export is a CLI program and a Rust library to export an [Obsidian] vault to regular Markdown.*

**English** | [简体中文](README.zh.md)

* Recursively export Obsidian Markdown files to [CommonMark].
* Supports `[[note]]`-style references as well as `![[note]]` file includes, including block references (`![[note#^block-id]]`) and same-file section embeds.
* Render diagram code blocks — dot (Graphviz), Mermaid, WaveDrom, TikZ — into image assets through local tools (`--render-diagrams`).
* Convert Obsidian `%%` comments to HTML comments — or strip them entirely — on export (`--comments`).
* Heading anchors match GitHub's slug algorithm, so `[[note#Section]]` links keep working on GitHub.
* Check a vault for broken links, missing sections and blocks without exporting anything (`obsidian-export check`).
* Self-update from GitHub releases (`obsidian-export update`).
* Support for [gitignore]-style exclude patterns (default: `.export-ignore`).
* Automatically excludes files that are ignored by Git when the vault is located in a Git repository.
* Runs on all major platforms: Windows, Mac, Linux, BSDs.
* A bilingual (Chinese/English) graphical desktop app ships alongside the CLI — see the [Desktop app](#desktop-app) section below.

Please note obsidian-export is not officially endorsed by the Obsidian team.
It supports most but not all of Obsidian's Markdown flavor.


# Installation

## Pre-built binaries

Pre-compiled CLI binaries, as well as the graphical desktop installer for Windows, are available at <https://github.com/ONEGAYI/obsidian-export-desktop/releases>

The desktop app bundles the CLI as its sidecar: installing the desktop app is enough, a separate CLI install is only needed if you want to use it from a terminal.

## Building from source

When binary releases are unavailable for your platform, or you do not trust the pre-built binaries, then *obsidian-export* can be compiled from source with relatively little effort.
This is done through [Cargo], the official package manager for Rust, with the following steps:

1. Install the Rust toolchain from <https://www.rust-lang.org/tools/install>
1. Clone this repository
1. Run `cargo install --path .` from the repository root

 > 
 > It is expected that you successfully configured the PATH variable correctly while installing the Rust toolchain, as described under *"Configuring the PATH environment variable"* on <https://www.rust-lang.org/tools/install>.

## Upgrading from earlier versions

If you downloaded a pre-built binary, upgrade by downloading the latest version to replace the old one — or let the CLI fetch it for you with `obsidian-export update --download`.

If you built from source, upgrade by pulling the latest changes and running `cargo install --path .` again.


# Basic usage

The main interface of *obsidian-export* is the `obsidian-export` CLI command.
As a text interface, this must be run from a terminal or Windows PowerShell.

It is assumed that you have basic familiarity with command-line interfaces and that you set up your `PATH` correctly if you installed with `cargo`.
Running `obsidian-export --version` should print a version number rather than giving some kind of error.

 > 
 > If you downloaded a pre-built binary and didn't put it a location referenced by `PATH` (for example, you put it in `Downloads`), you will need to provide the full path to the binary instead.
 > 
 > For example `~/Downloads/obsidian-export --version` on Mac/Linux or `~\Downloads\obsidian-export --version` on Windows (PowerShell).

## Exporting notes

In it's most basic form, `obsidian-export` takes just two mandatory arguments, a source and a destination:

````sh
obsidian-export /path/to/my-obsidian-vault /path/to/exported-notes/
````

This will export all of the files from `my-obsidian-vault` to `exported-notes`, except for those listed in `.export-ignore` or `.gitignore`.

 > 
 > Note that the destination directory must exist, so you may need to create a new, empty directory first.
 > 
 > If you give it an **existing** directory, files under that directory may get overwritten.

It is also possible to export individual files:

````sh
# Export as some-note.md to /tmp/export/
obsidian-export my-obsidian-vault/some-note.md /tmp/export/
# Export as exported-note.md in /tmp/
obsidian-export my-obsidian-vault/some-note.md /tmp/exported-note.md
````

Note that in this mode, obsidian-export sees `some-note.md` as being the only file that exists in your vault so references to other notes won't be resolved.
This is by design.

If you'd like to export a single note while resolving links or embeds to other areas in your vault then you should instead specify the root of your vault as the source, passing the file you'd like to export with `--start-at`, as described in the next section.

### Exporting a partial vault

Using the `--start-at` argument, you can export just a subset of your vault.
Given the following vault structure:

````
my-obsidian-vault
├── Notes/
├── Books/
└── People/
````

This will export only the notes in the `Books` directory to `exported-notes`:

````sh
obsidian-export my-obsidian-vault --start-at my-obsidian-vault/Books exported-notes
````

In this mode, all notes under the source (the first argument) are considered part of the vault so any references to these files will remain intact, even if they're not part of the exported notes.

## Checking links

The `check` command verifies every link in a vault without writing anything:

````sh
obsidian-export check /path/to/my-obsidian-vault
````

It walks the same file set an export would, and reports broken wikilinks and standard Markdown links — missing files, missing sections, missing block ids — one `source:line` entry per problem.
Links pointing outside the vault root are treated as broken; external URLs are ignored.
The exit code follows the usual convention (0 = all healthy, 1 = broken links found, 2 = usage error), so it fits right into scripts and CI.
Walk options like `--start-at`, `--hidden`, `--no-git` and `--ignore-file` apply here as well.

## Updating

The `update` command checks GitHub for a newer release and prints what it finds:

````sh
obsidian-export update
````

Add `--download` to also fetch the artifact — the CLI binary by default, or the Windows desktop installer with `--asset desktop` — into a temporary downloads directory (override with `--output`).
The check exits 0 either way; scripts can parse the machine-readable stream of `--progress json` to act on the result.

## Character encodings

At present, UTF-8 character encoding is assumed for all note text as well as filenames.
All text and file handling performs [lossy conversion to Unicode strings][from_utf8_lossy].

Use of non-UTF8 encodings may lead to issues like incorrect text replacement and failure to find linked notes.
While this may change in the future, there are no plans to change this behavior in the short term.


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

Obsidian renders special fenced code blocks (```` ```dot ````, ```` ```mermaid ````, …) through plugins, but plain Markdown consumers show them as literal code. With `--render-diagrams`, such blocks are rendered into standalone image files by shelling out to the corresponding local tools, and the export embeds a regular Markdown image reference instead:

````sh
obsidian-export --render-diagrams dot,mermaid,wavedrom,tikz SOURCE TARGET
````

Renderers and the external tools they require:

|Renderer|Code block languages|Requires|Formats|
|--------|--------------------|--------|-------|
|dot|`dot`, `graphviz`|[Graphviz](https://graphviz.org/download/) (`dot`)|svg, png|
|mermaid|`mermaid`, `mmd`|[mermaid-cli](https://github.com/mermaid-js/mermaid-cli) (`mmdc`)|svg, png|
|wavedrom|`wavedrom`|[wavedrom](https://www.npmjs.com/package/wavedrom)|svg|
|tikz|`tikz`|a TeX distribution with `latex` and `dvisvgm` (e.g. TeX Live)|svg|
|excalidraw|(whole drawing files, see below)|[excalidraw-export](https://www.npmjs.com/package/@moona3k/excalidraw-export)|svg, png|

### Excalidraw drawings

Unlike the fence renderers above, `excalidraw` converts whole drawing files rather than code blocks. It recognizes three shapes: legacy `.excalidraw` files (bare scene JSON), `.excalidraw.md` files (the plugin's default), and plain `.md` files whose frontmatter carries the `excalidraw-plugin` key (the plugin's Logseq-compatible shape).

With the renderer enabled, the export prescan converts every drawing under the export scope into an image at the source file's output-tree position, using the plugin's Auto-Export naming: `x.excalidraw.md` → `x.excalidraw.svg`, bare `.excalidraw` → `x.svg`; `--diagram-format png` switches to PNG. Embeds (`![[x.excalidraw.md]]`, `![[x.excalidraw]]`) become image references, plain links point at the converted asset, and the original drawing file itself is not exported. A same-named `.svg`/`.png` twin already sitting next to the drawing in the vault (e.g. a stale plugin Auto-Export) is skipped as well, so it cannot overwrite or orphan the freshly rendered asset. LaTeX formulas and pasted images travel inside the scene as data URLs and carry over without extra tooling.

Failures degrade rather than abort: a drawing the tool cannot render is not exported either, its embeds become plain links to the original vault path (kept for traceability) with an italic notice, and a warning is emitted; the export still completes. Drawings outside the `--start-at` scope behave the same way. Note that the converter is an independent reimplementation of the Excalidraw renderer (roughjs-based), so complex drawings may lose fidelity; a drop-in replacement can be supplied via `--diagram-bin excalidraw-export=/path/to/tool` as long as it accepts `IN [--svg] -o OUT`.

Behavior details:

* **Tool discovery** prefers an explicit path (`--diagram-bin dot=/path/to/dot`, repeatable) and otherwise scans `PATH`. On Windows the scan honors `PATHEXT` and runs npm's `.cmd` shims through `cmd.exe`, so global npm installs work out of the box. (The `cmd.exe` wrapper expands `%VARIABLE%`-shaped substrings even inside quotes: on the rare path containing a paired `%`, point `--diagram-bin` at the underlying `.exe` to bypass the wrapper; such tool paths are automatically warned about at export time.)
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

````
# Ignore the directory private that is located at the top of the export tree
/private
# Ignore any file or directory called `test`
test
# Ignore any PDF file
*.pdf
# ..but include special.pdf
!special.pdf
````

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

````
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
````

And `layouts/_default/_markup/render-image.html` for images:

````
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
````

With these hooks in place, links to both notes as well as file attachments should now work correctly.

 > 
 > Note: If you're using a theme which comes with it's own render hooks, you might need to do a little extra work, or customize the snippets above, to avoid conflicts with the hooks from your theme.


# Library usage

All of the functionality exposed by the `obsidian-export` CLI command is also accessible as a Rust library, exposed through the [`obsidian_export` crate][obsidian-export-crates-io].

To get started, visit the library documentation on [obsidian_export][crate-docs] and [obsidian_export::Exporter][exporter-docs].


# Desktop app

A graphical desktop app is available alongside the CLI. It wraps the exact same exporter: the GUI bundles the `obsidian-export` CLI as a sidecar process and shows its progress live, so anything the CLI can export, the desktop app can too.

Features include:

* Obsidian-styled light/dark themes (with a follow-system option) in a frameless window.
* Bilingual UI (Chinese / English) with a language menu: pick either language explicitly or follow the system locale; the choice is remembered across sessions.
* Folder pickers for the vault and destination, with the last-used paths remembered.
* A full conversion options view mirroring every CLI flag: frontmatter strategy, missing-section handling, Obsidian comment handling (keep/convert/strip), hard line breaks, recursive embeds, hidden files, git integration, the ignore-file name, skip/only tags, a start-at sub-path, mtime preservation and fail-fast. Options are remembered across sessions, and only non-default values are passed to the sidecar.
* Diagram rendering: dot (Graphviz), Mermaid, WaveDrom and TikZ code blocks can be rendered into image assets through local tools. The settings page shows the enabled renderers at a glance (with a count badge in the navigation) and manages them as pill-style checkboxes; the output format (svg/png, with per-renderer fallback) and per-tool executable paths (blank = PATH lookup) are configurable. Rendering progress ("diagram 3/12") shows in the run view; a missing tool aborts the export before anything is written.
* A pre-export sheet summarizing the effective options (with a shortcut back to the options view), and an option to export into `<destination>/<vault folder name>` so the vault's first-level entries stay contained.
* Live progress, per-file log lines, failure details with full error chains, and cancellation of a running export.
* An optional post-export link check (against the vault source or the exported tree) with a per-link report of broken links, missing sections and blocks.
* An "About & update" page: the app checks GitHub releases on launch (at most once a day, toggleable) and on demand, shows release notes, and can download and launch the new installer.

The CLI remains fully usable on its own; the desktop app is simply another way to invoke it.

For building or running the desktop app from source, see [`docs/BUILD.md`](docs/BUILD.md).


# Contributing

I will happily accept bug fixes as well as enhancements, as long as they align with the overall scope and vision of the project.
Please see [CONTRIBUTING](CONTRIBUTING.md) for more information.


# License

Obsidian-export is open-source software released under the [BSD-2-Clause Plus Patent License].
This license is designed to provide: a) a simple permissive license; b) that is compatible with the GNU General Public License (GPL), version 2; and c) which also has an express patent grant included.

Please review the [LICENSE] file for the full text of the license.


# Changelog

For a list of releases and the changes with each version, please refer to the [CHANGELOG](CHANGELOG.md).

[Obsidian]: https://obsidian.md/
[CommonMark]: https://commonmark.org/
[gitignore]: https://git-scm.com/docs/gitignore
[Cargo]: https://doc.rust-lang.org/cargo/
[from_utf8_lossy]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy
[Hugo]: https://gohugo.io
[hugo-relative-linking]: https://notes.nick.groenen.me/notes/relative-linking-in-hugo/
[`ref` and `relref` shortcodes]: https://gohugo.io/content-management/cross-references/
[Markdown Render Hooks]: https://gohugo.io/getting-started/configuration-markup#markdown-render-hooks
[obsidian-export-crates-io]: https://crates.io/crates/obsidian-export
[crate-docs]: https://docs.rs/obsidian-export/latest/obsidian_export/
[exporter-docs]: https://docs.rs/obsidian-export/latest/obsidian_export/struct.Exporter.html
[BSD-2-Clause Plus Patent License]: https://spdx.org/licenses/BSD-2-Clause-Patent.html
[LICENSE]: LICENSE
