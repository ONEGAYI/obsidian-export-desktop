# Obsidian Export

_Obsidian Export is a CLI program and a Rust library to export an [Obsidian] vault to regular Markdown._

**English** | [简体中文](README.zh.md)

- Recursively export Obsidian Markdown files to [CommonMark].
- Supports `[[note]]`-style references as well as `![[note]]` file includes, including block references (`![[note#^block-id]]`) and same-file section embeds.
- Render diagram code blocks — dot (Graphviz), Mermaid, WaveDrom, TikZ — into image assets through local tools (`--render-diagrams`).
- Convert Obsidian `%%` comments to HTML comments — or strip them entirely — on export (`--comments`).
- Heading anchors match GitHub's slug algorithm, so `[[note#Section]]` links keep working on GitHub.
- Check a vault for broken links, missing sections and blocks without exporting anything (`obsidian-export check`).
- Self-update from GitHub releases (`obsidian-export update`).
- Support for [gitignore]-style exclude patterns (default: `.export-ignore`).
- Automatically excludes files that are ignored by Git when the vault is located in a Git repository.
- Runs on all major platforms: Windows, Mac, Linux, BSDs.
- A bilingual (Chinese/English) graphical desktop app ships alongside the CLI — see the [Desktop app](#desktop-app) section below.

Please note obsidian-export is not officially endorsed by the Obsidian team.
It supports most but not all of Obsidian's Markdown flavor.

[Obsidian]: https://obsidian.md/
[CommonMark]: https://commonmark.org/
[gitignore]: https://git-scm.com/docs/gitignore
