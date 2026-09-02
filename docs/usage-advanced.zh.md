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

`--render-diagrams` 用户请注意：工具可用性预扫描会跟随 `--comments` 模式。位于 `%%` 注释内的图表代码块仅在 `--comments keep` 下计入计数并要求对应工具已安装；`strip` 与 `convert` 下该块不会进入渲染阶段，因此既不计数、也不要求安装工具。

## 失败文件

默认情况下，导出失败的笔记（例如 frontmatter YAML 损坏）会被记录，其余笔记继续导出；结束时打印汇总，列出所有失败笔记。使用 `--fail-fast` 则改为在第一个失败处停止。注意并行导出下，失败发生时已在处理中的文件仍会完成。

## 图表渲染

Obsidian 通过插件渲染特殊围栏代码块（` ```dot `、` ```mermaid ` 等），但普通 Markdown 消费者会把它们显示为字面代码。使用 `--render-diagrams` 后，这类代码块会经由对应的本机工具渲染为独立图片文件，导出产物以常规 Markdown 图片引用嵌入：

```sh
obsidian-export --render-diagrams dot,mermaid,wavedrom,tikz SOURCE TARGET
```

渲染器与所需外部工具：

| 渲染器 | 代码块语言 | 依赖 | 格式 |
|--------|--------------------|----------|---------|
| dot | `dot`、`graphviz` | [Graphviz](https://graphviz.org/download/)（`dot`） | svg、png |
| mermaid | `mermaid`、`mmd` | [mermaid-cli](https://github.com/mermaid-js/mermaid-cli)（`mmdc`） | svg、png |
| wavedrom | `wavedrom` | [wavedrom](https://www.npmjs.com/package/wavedrom) | svg |
| tikz | `tikz` | 含 `latex` 与 `dvisvgm` 的 TeX 发行版（如 TeX Live） | svg |

行为细节：

* **工具发现**优先显式路径（`--diagram-bin dot=/path/to/dot`，可重复），否则扫描 `PATH`。Windows 下扫描遵循 `PATHEXT`，并经 `cmd.exe` 运行 npm 的 `.cmd` shim，全局 npm 安装开箱即用。（`cmd.exe` 包装对引号内的 `%VARIABLE%` 形态子串也会做变量展开：路径中罕见地出现成对 `%` 时，可用 `--diagram-bin` 指向底层 `.exe` 绕过包装。）
* `--diagram-format` 与 `--diagram-bin` 仅在与 `--render-diagrams` 同用时生效；单独传递只会在 stderr 打一条警告并被忽略。
* **原子性**：工具解析发生在预扫描阶段（按真正会渲染的块的语言，见上文 `--comments` 说明——`strip`/`convert` 下位于 `%%` 注释内的块不要求工具），先于任何输出文件的写出。工具缺失时导出以退出码 1 中止并附安装建议，目标目录原封不动。
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

```
# 忽略位于导出树顶部的 private 目录
/private
# 忽略任何名为 `test` 的文件或目录
test
# 忽略所有 PDF 文件
*.pdf
# ..但保留 special.pdf
!special.pdf
```

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

图片则创建 `layouts/_default/_markup/render-image.html`：

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

配置好这两个 hook 后，指向笔记与文件附件的链接都应能正常工作。

> 注意：如果你使用的主题自带 render hook，可能需要额外处理或按需调整上述代码片段，以避免与主题的 hook 冲突。

[`ref` 与 `relref` shortcode]: https://gohugo.io/content-management/cross-references/
[gitignore]: https://git-scm.com/docs/gitignore
[hugo-relative-linking]: https://notes.nick.groenen.me/notes/relative-linking-in-hugo/
[hugo]: https://gohugo.io
[markdown render hooks]: https://gohugo.io/getting-started/configuration-markup#markdown-render-hooks
