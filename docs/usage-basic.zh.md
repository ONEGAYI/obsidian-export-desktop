# 基本用法

_obsidian-export_ 的主接口是 `obsidian-export` CLI 命令。
作为文本界面程序，它需要在终端或 Windows PowerShell 中运行。

这里假定你对命令行界面有基本了解，并且在使用 `cargo` 安装后已正确配置 `PATH`。
运行 `obsidian-export --version` 应当打印出版本号，而不是报某种错误。

> 如果你下载了预编译二进制但没有放到 `PATH` 引用的位置（例如留在了「下载」目录），就需要提供二进制的完整路径。
>
> 例如 Mac/Linux 上的 `~/Downloads/obsidian-export --version`，或 Windows（PowerShell）上的 `~\Downloads\obsidian-export --version`。

## 导出笔记

最基本的形式下，`obsidian-export` 只需两个必填参数：来源与目标：

```sh
obsidian-export /path/to/my-obsidian-vault /path/to/exported-notes/
```

这会把 `my-obsidian-vault` 中的全部文件导出到 `exported-notes`，`.export-ignore` 与 `.gitignore` 中列出的文件除外。

> 注意目标目录必须已存在，因此你可能需要先创建一个新的空目录。
>
> 如果给到一个**已存在**的目录，其下的文件可能被覆盖。

也支持导出单个文件：

```sh
# 导出为 /tmp/export/some-note.md
obsidian-export my-obsidian-vault/some-note.md /tmp/export/
# 导出为 /tmp/exported-note.md
obsidian-export my-obsidian-vault/some-note.md /tmp/exported-note.md
```

注意在此模式下，obsidian-export 视 `some-note.md` 为 vault 中唯一存在的文件，指向其他笔记的引用不会被解析。
这是有意为之。

若想在解析链接与嵌入的前提下导出单个笔记，应改为以 vault 根目录作为来源、用 `--start-at` 指定要导出的文件，见下一节。

### 导出部分 vault

使用 `--start-at` 参数可以只导出 vault 的一个子集。
给定如下 vault 结构：

```
my-obsidian-vault
├── Notes/
├── Books/
└── People/
```

下面这条命令只把 `Books` 目录中的笔记导出到 `exported-notes`：

```sh
obsidian-export my-obsidian-vault --start-at my-obsidian-vault/Books exported-notes
```

此模式下，来源（第一个参数）下的全部笔记都被视为 vault 的一部分，因此指向这些文件的引用保持完好，即使它们不在被导出的笔记之列。

## 链接检查

`check` 命令在不写出任何文件的前提下，校验 vault 中的每一个链接：

```sh
obsidian-export check /path/to/my-obsidian-vault
```

它遍历与导出一致的文件集，逐条报告失效的 wikilink 与标准 Markdown 链接——缺失文件、缺失章节、缺失块 id——每个问题一行 `source:line` 条目。
指向 vault 根之外的链接按失效处理；外部 URL 被跳过。
退出码遵循既有约定（0 = 全部健康，1 = 存在失效链接，2 = 参数错误），可直接用于脚本与 CI。
`--start-at`、`--hidden`、`--no-git`、`--ignore-file` 等遍历选项在这里同样可用。

## 更新

`update` 命令检查 GitHub 上是否有新版本并打印结果：

```sh
obsidian-export update
```

追加 `--download` 可同时下载产物——默认为 CLI 二进制，加 `--asset desktop` 则下载 Windows 桌面端安装包——落盘到临时下载目录（可用 `--output` 改写）。
无论有无更新，检查本身退出码均为 0；脚本可解析 `--progress json` 的机器可读事件流来处理结果。

## 字符编码

目前，所有笔记正文与文件名都假定为 UTF-8 编码。
全部文本与文件处理都执行[到 Unicode 字符串的有损转换][from_utf8_lossy]。

使用非 UTF-8 编码可能导致文本替换错误、链接笔记查找失败等问题。
这一行为将来可能改变，但短期内没有调整计划。

[from_utf8_lossy]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_lossy
