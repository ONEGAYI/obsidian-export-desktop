# Obsidian Export

_Obsidian Export 是一个 CLI 程序，同时也是一个 Rust 库，用于把 [Obsidian] 仓库导出为通用 Markdown。_

[English](README.md) | **简体中文**

- 递归导出 Obsidian Markdown 文件为 [CommonMark]。
- 支持 `[[note]]` 式引用与 `![[note]]` 文件嵌入，包括块引用（`![[note#^block-id]]`）与同文件章节嵌入。
- 将图表代码块——dot（Graphviz）、Mermaid、WaveDrom、TikZ——调用本机工具渲染为图片资产（`--render-diagrams`）。
- 导出时将 Obsidian `%%` 注释转换为 HTML 注释，或彻底移除（`--comments`）。
- 标题锚点对齐 GitHub 的 slug 算法，`[[note#Section]]` 链接在 GitHub 上依然可跳转。
- 无需导出即可检查 vault 的失效链接、缺失章节与块（`obsidian-export check`）。
- 从 GitHub releases 自检更新并下载（`obsidian-export update`）。
- 支持 [gitignore] 风格的排除模式（默认：`.export-ignore`）。
- vault 位于 Git 仓库内时，自动排除被 Git 忽略的文件。
- 运行于所有主流平台：Windows、Mac、Linux、BSD。
- 随 CLI 一并提供中英双语的图形界面桌面端——见下文[桌面端](#桌面端)章节。

请注意，obsidian-export 并未获得 Obsidian 团队的官方背书。
它支持 Obsidian Markdown 方言的大部分（但非全部）语法。

[Obsidian]: https://obsidian.md/
[CommonMark]: https://commonmark.org/
[gitignore]: https://git-scm.com/docs/gitignore
