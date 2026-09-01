# Basic usage

The main interface of _obsidian-export_ is the `obsidian-export` CLI command.
As a text interface, this must be run from a terminal or Windows PowerShell.

It is assumed that you have basic familiarity with command-line interfaces and that you set up your `PATH` correctly if you installed with `cargo`.
Running `obsidian-export --version` should print a version number rather than giving some kind of error.

> If you downloaded a pre-built binary and didn't put it a location referenced by `PATH` (for example, you put it in `Downloads`), you will need to provide the full path to the binary instead.
>
> For example `~/Downloads/obsidian-export --version` on Mac/Linux or `~\Downloads\obsidian-export --version` on Windows (PowerShell).

## Exporting notes

In it's most basic form, `obsidian-export` takes just two mandatory arguments, a source and a destination:

```sh
obsidian-export /path/to/my-obsidian-vault /path/to/exported-notes/
```

This will export all of the files from `my-obsidian-vault` to `exported-notes`, except for those listed in `.export-ignore` or `.gitignore`.

> Note that the destination directory must exist, so you may need to create a new, empty directory first.
>
> If you give it an **existing** directory, files under that directory may get overwritten.

It is also possible to export individual files:

```sh
# Export as some-note.md to /tmp/export/
obsidian-export my-obsidian-vault/some-note.md /tmp/export/
# Export as exported-note.md in /tmp/
obsidian-export my-obsidian-vault/some-note.md /tmp/exported-note.md
```

Note that in this mode, obsidian-export sees `some-note.md` as being the only file that exists in your vault so references to other notes won't be resolved.
This is by design.

If you'd like to export a single note while resolving links or embeds to other areas in your vault then you should instead specify the root of your vault as the source, passing the file you'd like to export with `--start-at`, as described in the next section.

### Exporting a partial vault

Using the `--start-at` argument, you can export just a subset of your vault.
Given the following vault structure:

```
my-obsidian-vault
├── Notes/
├── Books/
└── People/
```

This will export only the notes in the `Books` directory to `exported-notes`:

```sh
obsidian-export my-obsidian-vault --start-at my-obsidian-vault/Books exported-notes
```

In this mode, all notes under the source (the first argument) are considered part of the vault so any references to these files will remain intact, even if they're not part of the exported notes.

## Checking links

The `check` command verifies every link in a vault without writing anything:

```sh
obsidian-export check /path/to/my-obsidian-vault
```

It walks the same file set an export would, and reports broken wikilinks and standard Markdown links — missing files, missing sections, missing block ids — one `source:line` entry per problem.
Links pointing outside the vault root are treated as broken; external URLs are ignored.
The exit code follows the usual convention (0 = all healthy, 1 = broken links found, 2 = usage error), so it fits right into scripts and CI.
Walk options like `--start-at`, `--hidden`, `--no-git` and `--ignore-file` apply here as well.

## Updating

The `update` command checks GitHub for a newer release and prints what it finds:

```sh
obsidian-export update
```

Add `--download` to also fetch the artifact — the CLI binary by default, or the Windows desktop installer with `--asset desktop` — into a temporary downloads directory (override with `--output`).
The check exits 0 either way; scripts can parse the machine-readable stream of `--progress json` to act on the result.

## Character encodings

At present, UTF-8 character encoding is assumed for all note text as well as filenames.
All text and file handling performs [lossy conversion to Unicode strings][from_utf8_lossy].

Use of non-UTF8 encodings may lead to issues like incorrect text replacement and failure to find linked notes.
While this may change in the future, there are no plans to change this behavior in the short term.

[from_utf8_lossy]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy
