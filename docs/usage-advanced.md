# Advanced usage

## Frontmatter

By default, frontmatter is copied over "as-is".

Some static site generators are picky about frontmatter and require it to be present.
Some get tripped up when Markdown files don't have frontmatter but start with a list item or horizontal rule.
In these cases, `--frontmatter=always` can be used to insert an empty frontmatter entry.

To completely remove any frontmatter from exported notes, use `--frontmatter=never`.

## Missing sections

An embed or link pointing at a section (heading) that doesn't exist in the target note — including block references like `![[note#^block-id]]`, which never match a heading — is handled according to `--missing-section`:

* `--missing-section skip` (the default): the embed is replaced with nothing and a warning is emitted. Closest to Obsidian's own "not found" rendering.
* `--missing-section embed-full`: the entire note is embedded (the historical behavior of this tool).
* `--missing-section fail`: the export of the note containing the embed fails with an error.

The strategy is applied independently at every level of embedding: a missing section only affects that single embed, never the rest of the parent note.

## Failing files

By default, a note that fails to export (e.g. broken YAML frontmatter) is recorded and the export continues with the remaining notes; at the end, a summary listing every failing note is printed. Use `--fail-fast` to instead stop on the first failing file. Note that with parallel exports, files already being processed when the failure occurs may still complete.

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
