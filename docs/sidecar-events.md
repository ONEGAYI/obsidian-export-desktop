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
  stdout 管道提前关闭）；`2` 参数错误。不存在其他退出码（含 panic 的 101）。
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
