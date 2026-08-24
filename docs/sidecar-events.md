# 边车事件契约（`--progress json`）

> 面向桌面端（GUI）开发者的集成契约。GUI 不实现任何转换逻辑，一律通过启动
> obsidian-export 子进程、传参并消费其输出完成导出。本文不进入 README 生成
> 列表（`docs/_combined.md`），因为它是本项目桌面端改造的本地文档，不属于
> 上游 CLI 用户文档。

## 调用约定

- GUI 以子进程方式启动 CLI，`--progress json` 开启机器可读输出。
- stdout 只承载 JSON Lines 事件流（每行一个合法 JSON 对象）， stderr 承载
  人类可读的错误报告与提示。两者严格分流。
- 退出码：`0` 成功；`1` 运行时错误（含部分文件失败、fail-fast 中止、
  stdout 管道提前关闭；check 语境下还含「发现失效链接」）；`2` 参数错误。
  不应出现其他退出码（含 panic 的 101——出现即缺陷，请反馈）。
- 启动时建议先运行 `--version`（stdout 单行 `obsidian-export x.y.z`，
  退出码 0）校验边车版本，再决定是否启用新事件字段。

## 事件流

首行恒为 schema 声明，随后按发生顺序输出事件，末行恒为 `end`：

| type | 字段 | 说明 |
|------|------|------|
| `schema` | `version: number` | 当前为 `1`。字段增删或语义变化时递增 |
| `start` | `total: number` | 待处理文件总数 |
| `file-done` | `path: string` | 一个文件导出成功 |
| `file-skipped` | `path: string` | 文件被后处理器跳过（如 `--skip-tags`） |
| `file-failed` | `path: string`, `message: string` | 文件导出失败；`message` 为完整错误链（外层：内层：根因）。默认策略下其余文件继续 |
| `warning` | `path: string \| null`, `message: string` | 非致命警告（死链、缺失章节等）；`path` 为警告来源笔记，无法确定时为 `null` |
| `end` | `failed: string[]` | 流终止。列出失败文件的来源路径 |

除首尾外的事件顺序不保证（导出并行执行）；`start` 之后、`end` 之前，
`file-*` 与 `warning` 可能交错。

## 终止协议

**`start` 发射之后的每一种结束路径都会发射 `end`**：全部成功、部分失败
（`failed` 非空）、fail-fast 中止（`failed` 含中止时已知的失败文件，
可能有并发在飞的多于一个）、单文件导出失败。

GUI 的状态机因此可以单一判定：**收到 `end` 即本次运行终结**（成败看
`failed` 与退出码）；**退出但未收到 `end` = 运行未进入处理阶段或边车
异常崩溃**——参数错误（退出码 2）连 schema 行都没有；schema 行已出但
无 `start`/`end` 的是前置校验失败（如 root 路径不存在、`--start-at`
越界），原因在 stderr。

注意：GUI 提前关闭 stdout 管道（如用户取消）会让 CLI 以退出码 `1`
安静退出，不再产生后续事件。

## 路径约定

- 事件中的 `path` 是**来源文件路径**，形态与传给 CLI 的 `source` 参数
  一致（传入绝对路径则事件为绝对路径），分隔符为平台原生风格
  （Windows 下为 `\`）。
- **GUI 应始终传规范化的绝对路径**，这样事件路径可直接显示与定位；
  混用分隔符传入将原样反映到事件中。

## 已知行为说明

- `--fail-fast` 首错后停止调度新任务，但并发在飞的文件仍会完成并照常
  上报（`file-done` 可能出现在失败之后）。
- 非 UTF-8 参数经无损替换（U+FFFD）后按路径处理，通常报「路径不存在」
  （退出码 1）。Windows GUI 的 UTF-16 参数不受影响。
- 默认（不带 `--progress json`）stdout 完全静默，警告与错误走 stderr，
  与上游行为一致。

## check 子命令事件流

`obsidian-export check SOURCE --progress json` 输出独立于导出流的第二种
事件方言（dialect），**共享同一 schema 版本常量与版本协商机制**：首行
同样恒为 `schema`（`version: 1`）。两种方言的事件类型互不相识——消费端
用各自的解析器，未知类型一律忽略（前向兼容）。

| type | 字段 | 说明 |
|------|------|------|
| `schema` | `version: number` | 同导出流，当前为 `1` |
| `check-start` | `files: number` | 参与检查的文件数 |
| `link-report` | `source: string`, `line: number`, `raw: string`, `kind`, `status` | 一条链接的检查结论。`source` 为相对检查根的路径（`/` 分隔），`line` 1-based（不可读文件的占位条目为 0），`raw` 为链接原文；`kind`/`status` 见下 |
| `check-end` | `filesChecked: number`, `totalLinks: number`, `broken: number`, `skipped: number` | 汇总终止行 |

`kind` 取值：`wiki-link` / `wiki-embed` / `markdown-link` /
`markdown-image`（未来未知变体降级为 `unknown`，不丢整条报告）。

`status` 为对象形态，`type` 取值与载荷：

| type | 载荷 | 含义 |
|------|------|------|
| `ok` | — | 链接有效 |
| `missing-file` | `target` | 目标文件不存在 |
| `out-of-bounds` | `target` | 链接越出检查根（即使盘上存在也判失效） |
| `missing-section` | `target`, `section` | 目标文件中不存在该章节 |
| `missing-block` | `target`, `block` | 目标文件中不存在该块 id |
| `file-unreadable` | `message` | 源文件不可读（占位条目） |
| `external-skipped` | `url` | 外部 URL，有意跳过 |

**broken 的定义**：`status.type` 既非 `ok` 也非 `external-skipped`。

### check 特有语义

- **输出时序**：`Exporter::check()` 一次性返回完整汇总，`link-report`
  逐条在检查完成后批量输出（非逐文件流式）；GUI 按行消费无感知差异，
  但不要把「事件陆续到达」当作进度信号——进度可用的只有 `check-start`
  的文件数与已到达的报告条数。
- **退出码**：`0` 无失效；`1` 有任何失效 **或 check 本身失败**——两
  者靠「有无 `check-end`」区分（有 = 正常完成，无 = 运行失败，原因在
  stderr）；`2` 参数错误（连 schema 行都没有）。
- **检查根选择**：check 的 SOURCE 可以是 vault 源，也可以是导出产物
  文件夹（产物内是百分号编码的标准 markdown 链接，check 有意支持）。
  注意两者结论不等价：导出时死链 wikilink 已塌缩为斜体纯文本，检查
  产物抓不到它们；检查 vault 源才能发现转换前的死链。
- **过滤参数**：对 vault 源检查时，GUI 转发与导出一致的非默认过滤项
  （`--start-at`/`--ignore-file`/`--hidden`/`--no-git`），保证检查的
  **walk 集**与导出的 walk 集一致（`--skip-tags`/`--only-tags` 是导出
  后处理，被其过滤的文件不在产物中、链接仍会被检查，属有意行为）。
  对导出产物检查时不重放 vault 过滤项，但 CLI 的**默认值本身就是
  过滤**：必须恒传 `--no-git`（产物目录位于 git 仓库内时 gitignore
  会静默排除产物树，产生「全部健康」的假阴性），并使 `--hidden` 与
  导出一致（导出开启时产物含隐藏文件）。
