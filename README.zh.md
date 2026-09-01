<!--

WARNING：

  请勿直接编辑 README.zh.md，它由 docs 目录中的文件自动生成。

  请改为编辑 docs 目录中对应的 Markdown 文件（`.zh.md` 为中文源），然后运行
  generate.sh。

  新增章节时，在 docs 下创建新文件并将其加入 _combined.zh.md

-->


# Obsidian Export

*Obsidian Export 是一个 CLI 程序，同时也是一个 Rust 库，用于把 [Obsidian] 仓库导出为通用 Markdown。*

[English](README.md) | **简体中文**

* 递归导出 Obsidian Markdown 文件为 [CommonMark]。
* 支持 `[[note]]` 式引用与 `![[note]]` 文件嵌入，包括块引用（`![[note#^block-id]]`）与同文件章节嵌入。
* 将图表代码块——dot（Graphviz）、Mermaid、WaveDrom、TikZ——调用本机工具渲染为图片资产（`--render-diagrams`）。
* 导出时将 Obsidian `%%` 注释转换为 HTML 注释，或彻底移除（`--comments`）。
* 标题锚点对齐 GitHub 的 slug 算法，`[[note#Section]]` 链接在 GitHub 上依然可跳转。
* 无需导出即可检查 vault 的失效链接、缺失章节与块（`obsidian-export check`）。
* 从 GitHub releases 自检更新并下载（`obsidian-export update`）。
* 支持 [gitignore] 风格的排除模式（默认：`.export-ignore`）。
* vault 位于 Git 仓库内时，自动排除被 Git 忽略的文件。
* 运行于所有主流平台：Windows、Mac、Linux、BSD。
* 随 CLI 一并提供中英双语的图形界面桌面端——见下文[桌面端](#桌面端)章节。

请注意，obsidian-export 并未获得 Obsidian 团队的官方背书。
它支持 Obsidian Markdown 方言的大部分（但非全部）语法。


# 安装

## 预编译二进制

CLI 的预编译产物，以及 Windows 图形界面桌面端安装包，均可在 <https://github.com/ONEGAYI/obsidian-export-desktop/releases> 下载。

桌面端已内置 CLI 作为边车进程：安装桌面端即可直接使用，只有在想从终端调用 CLI 时才需要单独安装。

## 从源码构建

当你的平台没有预编译产物，或者你不信任预编译二进制时，*obsidian-export* 也可以轻松地从源码编译。
通过 Rust 官方包管理器 [Cargo] 完成，步骤如下：

1. 从 <https://www.rust-lang.org/tools/install> 安装 Rust 工具链
1. 克隆本仓库
1. 在仓库根目录运行 `cargo install --path .`

 > 
 > 安装 Rust 工具链时应按 <https://www.rust-lang.org/tools/install> 上「Configuring the PATH environment variable」一节的说明正确配置 PATH 变量。

## 从旧版本升级

下载了预编译二进制的用户，下载最新版本替换旧文件即可——也可以让 CLI 代劳：`obsidian-export update --download`。

从源码构建的用户，拉取最新代码后再次运行 `cargo install --path .` 即可。


# 基本用法

*obsidian-export* 的主接口是 `obsidian-export` CLI 命令。
作为文本界面程序，它需要在终端或 Windows PowerShell 中运行。

这里假定你对命令行界面有基本了解，并且在使用 `cargo` 安装后已正确配置 `PATH`。
运行 `obsidian-export --version` 应当打印出版本号，而不是报某种错误。

 > 
 > 如果你下载了预编译二进制但没有放到 `PATH` 引用的位置（例如留在了「下载」目录），就需要提供二进制的完整路径。
 > 
 > 例如 Mac/Linux 上的 `~/Downloads/obsidian-export --version`，或 Windows（PowerShell）上的 `~\Downloads\obsidian-export --version`。

## 导出笔记

最基本的形式下，`obsidian-export` 只需两个必填参数：来源与目标：

````sh
obsidian-export /path/to/my-obsidian-vault /path/to/exported-notes/
````

这会把 `my-obsidian-vault` 中的全部文件导出到 `exported-notes`，`.export-ignore` 与 `.gitignore` 中列出的文件除外。

 > 
 > 注意目标目录必须已存在，因此你可能需要先创建一个新的空目录。
 > 
 > 如果给到一个**已存在**的目录，其下的文件可能被覆盖。

也支持导出单个文件：

````sh
# 导出为 /tmp/export/some-note.md
obsidian-export my-obsidian-vault/some-note.md /tmp/export/
# 导出为 /tmp/exported-note.md
obsidian-export my-obsidian-vault/some-note.md /tmp/exported-note.md
````

注意在此模式下，obsidian-export 视 `some-note.md` 为 vault 中唯一存在的文件，指向其他笔记的引用不会被解析。
这是有意为之。

若想在解析链接与嵌入的前提下导出单个笔记，应改为以 vault 根目录作为来源、用 `--start-at` 指定要导出的文件，见下一节。

### 导出部分 vault

使用 `--start-at` 参数可以只导出 vault 的一个子集。
给定如下 vault 结构：

````
my-obsidian-vault
├── Notes/
├── Books/
└── People/
````

下面这条命令只把 `Books` 目录中的笔记导出到 `exported-notes`：

````sh
obsidian-export my-obsidian-vault --start-at my-obsidian-vault/Books exported-notes
````

此模式下，来源（第一个参数）下的全部笔记都被视为 vault 的一部分，因此指向这些文件的引用保持完好，即使它们不在被导出的笔记之列。

## 链接检查

`check` 命令在不写出任何文件的前提下，校验 vault 中的每一个链接：

````sh
obsidian-export check /path/to/my-obsidian-vault
````

它遍历与导出一致的文件集，逐条报告失效的 wikilink 与标准 Markdown 链接——缺失文件、缺失章节、缺失块 id——每个问题一行 `source:line` 条目。
指向 vault 根之外的链接按失效处理；外部 URL 被跳过。
退出码遵循既有约定（0 = 全部健康，1 = 存在失效链接，2 = 参数错误），可直接用于脚本与 CI。
`--start-at`、`--hidden`、`--no-git`、`--ignore-file` 等遍历选项在这里同样可用。

## 更新

`update` 命令检查 GitHub 上是否有新版本并打印结果：

````sh
obsidian-export update
````

追加 `--download` 可同时下载产物——默认为 CLI 二进制，加 `--asset desktop` 则下载 Windows 桌面端安装包——落盘到临时下载目录（可用 `--output` 改写）。
无论有无更新，检查本身退出码均为 0；脚本可解析 `--progress json` 的机器可读事件流来处理结果。

## 字符编码

目前，所有笔记正文与文件名都假定为 UTF-8 编码。
全部文本与文件处理都执行[到 Unicode 字符串的有损转换][from_utf8_lossy]。

使用非 UTF-8 编码可能导致文本替换错误、链接笔记查找失败等问题。
这一行为将来可能改变，但短期内没有调整计划。


# 高级用法

## Frontmatter

默认情况下，frontmatter 原样复制。

一些静态站点生成器对 frontmatter 挑剔：有的要求必须存在，有的在 Markdown 文件缺少 frontmatter 却以列表项或水平线开头时会出问题。
这类场景可用 `--frontmatter=always` 插入一条空的 frontmatter。

要完全去除导出笔记中的 frontmatter，使用 `--frontmatter=never`。

## 缺失章节

指向不存在章节（标题）的嵌入，按 `--missing-section` 指定的策略处理：

* `--missing-section skip`（默认）：嵌入替换为空并发出警告。最接近 Obsidian 自身的「未找到」渲染。
* `--missing-section embed-full`：嵌入整篇笔记（本工具的历史行为）。
* `--missing-section fail`：包含该嵌入的笔记导出失败并报错。

该策略在每一层嵌入上独立生效：一个缺失章节只影响那一次嵌入，不影响父笔记的其余部分。

块引用（`![[note#^block-id]]`）会真实定位 id 所标记的块（一个段落、一个列表项或整个引用块；单独成行的 id 标记其上方的块）。`--missing-section` 策略同样覆盖 id 不存在的块引用。嵌入副本会剥离 id 标记——Obsidian 不显示它——而源笔记中的 id 定义保持原样。

同文件章节与块嵌入（`![[#Heading]]` / `![[#^block-id]]`）亦受支持，但有两个注意点：同文件嵌入内部的引用只针对被嵌入的切片解析（位于笔记其他位置的章节在切片内找不到，按 `--missing-section` 降级）；同一文件的展开内容中再出现同文件嵌入时降级为普通链接（该判定是文件级的，因此也包括本可安全展开的对其他章节的同文件引用）。

## Obsidian 注释

Obsidian 注释（`%%像这样%%`，含跨多行的块注释）只在编辑视图可见。默认按字面保留，通用 Markdown 渲染器会显示为 `%%` 文本。使用 `--comments` 选择处理方式：

* `--comments keep`（默认）：注释保持 `%%...%%` 字面。
* `--comments convert`：每条注释转换为 HTML 注释（`<!-- ... -->`），源码保留但渲染时隐藏。
* `--comments strip`：从输出中彻底移除注释。

识别遵循 Obsidian 的纯文本配对：第一个 `%%` 与下一个 `%%` 配对——即使跨越空行、列表或引用块边界；未闭合的 `%%` 保持字面。代码块、行内代码、数学公式、表格与链接文本内的 `%%` 不会被视为注释标记——与 Obsidian 自身不解释该语法的位置一致。会破坏 HTML 注释语法的内容会被中和（`--` 变为 `- -`）。

跨越块级边界的注释会把周围结构在注释处断开（例如列表项结束、HTML 注释作为独立块跟随、剩余列表在下方重启）；完全位于一个段落内的注释原位改写。被打断的有序列表在重启处回到起始编号——CommonMark 的列表语法不携带「当前序号」。

`--render-diagrams` 用户请注意：工具可用性预扫描在注释移除**之前**基于原始笔记文本执行。位于 `%%` 注释内的图表代码块仍要求对应工具已安装（即使 `--comments strip` 会把该块从产物中删除）；工具齐备时该块只是不渲染，且图表总数会把它计入。

## 失败文件

默认情况下，导出失败的笔记（例如 frontmatter YAML 损坏）会被记录，其余笔记继续导出；结束时打印汇总，列出所有失败笔记。使用 `--fail-fast` 则改为在第一个失败处停止。注意并行导出下，失败发生时已在处理中的文件仍会完成。

## 图表渲染

Obsidian 通过插件渲染特殊围栏代码块（```` ```dot ````、```` ```mermaid ```` 等），但普通 Markdown 消费者会把它们显示为字面代码。使用 `--render-diagrams` 后，这类代码块会经由对应的本机工具渲染为独立图片文件，导出产物以常规 Markdown 图片引用嵌入：

````sh
obsidian-export --render-diagrams dot,mermaid,wavedrom,tikz SOURCE TARGET
````

渲染器与所需外部工具：

|渲染器|代码块语言|依赖|格式|
|---|-----|--|--|
|dot|`dot`、`graphviz`|[Graphviz](https://graphviz.org/download/)（`dot`）|svg、png|
|mermaid|`mermaid`、`mmd`|[mermaid-cli](https://github.com/mermaid-js/mermaid-cli)（`mmdc`）|svg、png|
|wavedrom|`wavedrom`|[wavedrom](https://www.npmjs.com/package/wavedrom)|svg|
|tikz|`tikz`|含 `latex` 与 `dvisvgm` 的 TeX 发行版（如 TeX Live）|svg|

行为细节：

* **工具发现**优先显式路径（`--diagram-bin dot=/path/to/dot`，可重复），否则扫描 `PATH`。Windows 下扫描遵循 `PATHEXT`，并经 `cmd.exe` 运行 npm 的 `.cmd` shim，全局 npm 安装开箱即用。（`cmd.exe` 包装对引号内的 `%VARIABLE%` 形态子串也会做变量展开：路径中罕见地出现成对 `%` 时，可用 `--diagram-bin` 指向底层 `.exe` 绕过包装。）
* `--diagram-format` 与 `--diagram-bin` 仅在与 `--render-diagrams` 同用时生效；单独传递只会在 stderr 打一条警告并被忽略。
* **原子性**：工具解析发生在预扫描阶段（按 vault 中实际出现的语言），先于任何输出文件的写出。工具缺失时导出以退出码 1 中止并附安装建议，目标目录原封不动。
* **单块失败非致命**：工具拒绝其语法的图表保持代码块原样并产生警告；导出总是完成。
* **输出格式**：`--diagram-format png` 请求位图输出；不具备该能力的渲染器（wavedrom、tikz）回落 svg 并警告。
* **资产**写在每篇笔记旁的 `assets/<note>-<hash>.<ext>`（文件名按渲染器+语言+源码+格式做内容寻址），内容未变的块跨运行解析到同一文件，再次导出完全跳过外部工具。
* **tikz** 代码块内容是 `tikzpicture` 环境的*内部*（Obsidian 插件惯例）；自带 `\begin{tikzpicture}` 的源码原样嵌入。字体转换为路径（`dvisvgm --no-fonts`）以兼容渲染器；tikz 图内的 CJK 文本可能渲染异常——此类内容建议改用 mermaid 或 dot。

## 进度事件

传入 `--progress json` 会在 stdout 上以 JSON Lines 输出机器可读的进度事件，每行一个 JSON 对象。这面向以子进程方式驱动 obsidian-export 的程序：首行声明 schema 版本，随后是逐文件进度、警告与最终的 end 事件。不传此参数时 stdout 保持静默。

## 忽略文件

以下文件默认不导出：

* 隐藏文件（可用 `--hidden` 调整）
* 匹配 `.export-ignore` 所列模式的文件（可用 `--ignore-file` 调整）
* 被 Git 忽略的文件（可用 `--no-git` 调整）
* 使用 `--skip-tags foo --skip-tags bar` 会跳过 frontmatter 中带有 `foo` 或 `bar` 标签的文件
* 使用 `--only-tags foo --only-tags bar` 会跳过 frontmatter 中**不**带 `foo` 或 `bar` 标签的文件

（更多信息见 `--help`。）

指向被忽略笔记的链接会被解除链接化（只保留链接文字）。
被忽略笔记的嵌入会被整体跳过。

### 忽略文件语法

`.export-ignore` 的语法与 [gitignore] 文件完全一致。
示例：

````
# 忽略位于导出树顶部的 private 目录
/private
# 忽略任何名为 `test` 的文件或目录
test
# 忽略所有 PDF 文件
*.pdf
# ..但保留 special.pdf
!special.pdf
````

更完整的文档与示例见 [gitignore] manpage。

## 递归嵌入

两篇笔记互相嵌入时会形成「递归嵌入」。
例如 `Note A.md` 含 `![[Note B]]`，而 `Note B.md` 又含 `![[Note A]]`。

默认情况下这会触发错误，并展示造成递归的笔记链。

该行为可用 `--no-recursive-embeds` 改变。
此模式下，处理原笔记时第二次遇到的笔记不再重复嵌入，而是插入指向该笔记的链接以打破循环。

## Hugo 的相对链接

[Hugo] 静态站点生成器[不支持文件的相对链接][hugo-relative-linking]，
而是期望你用 [`ref` 与 `relref` shortcode] 链接其他页面。

因此，用 obsidian-export 从 Obsidian 导出的笔记无法开箱即用，因为 Hugo 不能正确解析这些链接。

不过，[Markdown Render Hooks]（仅默认的 `goldmark` 渲染器支持）可以绕过这一问题，稍作一次性配置后即可让导出笔记在 Hugo 中正常工作。

创建 `layouts/_default/_markup/render-link.html`，内容如下：

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

图片则创建 `layouts/_default/_markup/render-image.html`：

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

配置好这两个 hook 后，指向笔记与文件附件的链接都应能正常工作。

 > 
 > 注意：如果你使用的主题自带 render hook，可能需要额外处理或按需调整上述代码片段，以避免与主题的 hook 冲突。


# 库用法

`obsidian-export` CLI 命令暴露的全部功能，同样可以经由 [`obsidian_export` crate][obsidian-export-crates-io] 作为 Rust 库使用。

入门请参阅 [obsidian_export][crate-docs] 与 [obsidian_export::Exporter][exporter-docs] 的库文档。


# 桌面端

CLI 之外另提供图形界面桌面端。它包装的是同一个导出器：GUI 将 `obsidian-export` CLI 作为边车进程内置，并实时展示其进度——CLI 能导出的，桌面端都能导出。

功能包括：

* Obsidian 风格的明暗双主题（含跟随系统选项），无边框窗口。
* 中英双语界面与语言菜单：可显式选择任一语言或跟随系统语言，选择跨会话记忆。
* vault 与目标目录的文件夹选择器，记忆最近使用的路径。
* 完整对照全部 CLI 选项的转换选项视图：frontmatter 策略、缺失章节处理、Obsidian 注释处理（保留/转换/移除）、硬换行、递归嵌入、隐藏文件、Git 集成、忽略文件名、skip/only 标签、start-at 子路径、mtime 保留与 fail-fast。选项跨会话记忆，且仅非默认值会传给边车。
* 图表渲染：dot（Graphviz）、Mermaid、WaveDrom、TikZ 代码块可经本机工具渲染为图片资产。设置页一览已启用的渲染器（导航带计数 badge），以药丸式复选框管理它们；输出格式（svg/png，按渲染器回落）与逐工具可执行文件路径（留空 = PATH 查找）均可配置。运行视图展示渲染进度（如「图表 3/12」）；工具缺失时在写出任何文件之前中止导出。
* 导出前摘要面板（可快捷返回选项视图），以及「导出到 `<目标>/<vault 文件夹名>`」的选项，让 vault 的一级条目保持收纳。
* 实时进度、逐文件日志行、含完整错误链的失败详情，以及运行中导出的取消。
* 可选的导出后链接检查（对 vault 源或导出产物），逐链接报告失效链接、缺失章节与块。
* 「关于与更新」页：启动时（至多每天一次，可关闭）与按需检查 GitHub releases，展示 release notes，并可直接下载并启动新安装包。

CLI 依旧完全独立可用；桌面端只是调用它的另一种方式。

从源码构建或运行桌面端，见 [`docs/BUILD.md`](docs/BUILD.md)（中文）。


# 参与贡献

只要符合项目的整体范围与愿景，我乐于接受缺陷修复与功能增强。
更多信息请参阅 [CONTRIBUTING](CONTRIBUTING.md)（英文）。


# 许可证

Obsidian-export 是开源软件，采用 [BSD-2-Clause Plus Patent License] 发布。
该许可证旨在提供：a) 一个宽松简单的许可；b) 与 GNU 通用公共许可证（GPL）第 2 版兼容；c) 附带明确的专利授权。

许可证全文请查阅 [LICENSE] 文件（英文原文为准）。


# 变更日志

各版本的发布与变更列表，请参阅 [CHANGELOG](CHANGELOG.md)（英文）。

[Obsidian]: https://obsidian.md/
[CommonMark]: https://commonmark.org/
[gitignore]: https://git-scm.com/docs/gitignore
[Cargo]: https://doc.rust-lang.org/cargo/
[from_utf8_lossy]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy
[Hugo]: https://gohugo.io
[hugo-relative-linking]: https://notes.nick.groenen.me/notes/relative-linking-in-hugo/
[`ref` 与 `relref` shortcode]: https://gohugo.io/content-management/cross-references/
[Markdown Render Hooks]: https://gohugo.io/getting-started/configuration-markup#markdown-render-hooks
[obsidian-export-crates-io]: https://crates.io/crates/obsidian-export
[crate-docs]: https://docs.rs/obsidian-export/latest/obsidian_export/
[exporter-docs]: https://docs.rs/obsidian-export/latest/obsidian_export/struct.Exporter.html
[BSD-2-Clause Plus Patent License]: https://spdx.org/licenses/BSD-2-Clause-Patent.html
[LICENSE]: LICENSE
