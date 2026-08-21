# obsidian-export 修复完善计划（桌面端前置）

## 总原则

- 严格按 **M0 前置 → M1 正确性 → M2 稳健性 → M3 性能** 推进，上一阶段测试全绿才进下一阶段。
- 每项修复**测试先行**（TDD）：先写失败测试，再实现，golden 期望文件随之更新。
- 彻底完善前**不动桌面端业务**；core 改造即桌面端适配——所有新选项同时暴露为库 builder API 与 CLI 参数，枚举值字符串稳定可序列化，为 M2 的结构化输出铺路。
- GUI 极简原则：预检、警告聚合、panic 恢复等一律修在 core/CLI，桌面端只做展示。
- 每阶段独立分支 + 中文提交（`类型: 简述` + 正文）；远端策略未定前仅本地分支提交。

## M0 前置（环境护栏，先行合入）

1. **`.gitattributes`**：`* text=auto`，`tests/testdata/** eol=lf` 固定——修复 Windows 上 9/18 集成测试因 CRLF 检出失败的问题（换行符差异，非逻辑回归）。
2. **AGENTS.md 修正**：项目定位中「双链、嵌入、callout 等」改为实际支持的语法范围（callout 不转换，原样保留）；「待定事项」登记两条：块引用内容提取增强、serde_yaml 归档依赖迁移。
3. **`Cargo.toml`** 声明 `rust-version = "1.80"`（MSRV 机器可读化）。
4. 验收：Windows 本机 `cargo test` 全绿。

## M1 正确性（行为与预期不符的缺陷）

| # | 问题 | 方案 | 落点 |
|---|------|------|------|
| 1.1 | `[[note\|]]`、`[[#]]` 等空引用 panic | 修正正则使空 label/section 可匹配（`label` 组允许空），`from_str` 永不 panic，行为显式定义 | `src/references.rs:74-76` |
| 1.2 | 中文章节锚点音译失效 | 新写 `format_anchor`（小写、空格→`-`、去 ASCII 标点、保留 CJK、压缩连续 `-`），移除 slug 依赖 | `src/lib.rs:783` |
| 1.3 | section 找不到时静默嵌入整篇 | 新增 `MissingSectionStrategy`（`embed-full`/`skip`/`fail`，**默认 skip**）：库 `Exporter::missing_section_strategy()` + CLI `--missing-section` | `src/lib.rs:892` `reduce_to_section`、`embed_file` |
| 1.4 | 同名文件裸名引用解析不确定 | vault_contents 返回前排序；lookup tie-break：完整路径精确匹配优先 → 组件数最少 → 字典序 | `src/lib.rs:807-828`、`src/walker.rs` |
| 1.5 | 文件名含 `#` 的链接被截断 | percent-encode 集追加 `#` | `src/lib.rs:136` |
| 1.6 | 裸文件名 destination 误报空路径 | parent 为空字符串时视为当前目录（已存在） | `src/lib.rs:381-388` |
| 1.7 | `filter_by_tags` 漏标量形式 | `tags: foo` 标量与逗号分隔字符串归一化为列表再过滤 | `src/postprocessors.rs:27-31` |
| 1.8 | 杂项 | `Exporter` Debug 结构名误写修正；`![[img\|300]]` 数字尺寸 label 回退为文件名 alt；`copy_mtime` 错误补 `FileExportError` 上下文 | `src/lib.rs:252`、`:711` 附近、`:428` |

**1.3 的嵌套嵌入防卫语义**（针对你的忧虑，作为契约测试固定下来）：

- 策略在**每一层嵌入独立应用**：任何一层找不到目标 section/块，只影响该层——`skip` 则该层置空并发警告，**父层与其余内容继续正常导出**；`fail` 则错误归属到发起嵌入的文件。
- 深度与环防护不变且正交：`file_tree` 环检测 + `NOTE_RECURSION_LIMIT=10` 仍负责防无限递归，策略不改变其行为。
- 专项测试：A 嵌入 `B#缺失` 而 B 又嵌入 C 的三层嵌套，三策略 × 内层缺失/外层缺失矩阵。

**同批处理**：块引用 `![[note#^block]]` 不再静默整篇嵌入，匹配不到时走同一策略；代码处留 `TODO` 注释指向未来的块内容提取增强（已登记 AGENTS.md 待定事项）。

## M2 稳健性（错误处理与桌面端契约）

1. **panic 路径清零**：frontmatter 非 Text 事件等库内 panic（`src/lib.rs:519`、627、633）转为 `ExportError` 新变体；两处 `to_str().unwrap()`（673、756）改 `to_string_lossy`；main.rs `env::args` → `args_os`、`unwrap` → 带说明 `expect`。
2. **错误聚合**：`run()` 收集全部逐文件错误，**默认继续导出**，结束时返回含文件清单的汇总错误；新增 `--fail-fast` 恢复严格模式；循环嵌入 `RecursionLimitExceeded` 降级为单文件错误；CLI 汇总打印。
3. **事件回调 API**：`Exporter::on_event()` 注册回调，`ExportEvent` 枚举：`Start{total}` / `FileDone{path}` / `FileSkipped{path}` / `FileFailed{path, error}` / `Warning{..}` / `End{summary}`；两处硬编码 `eprintln` 警告（`src/lib.rs:669`、752，原 TODO 预留）改为事件，CLI 默认行为保持打印兼容。
4. **JSON Lines 输出**：CLI `--progress json` 时 stdout 输出单一事件流（进度+警告+错误+摘要），首事件携带 schema 版本；默认关闭保持静默兼容。
5. **CLI 契约测试**：新增 `tests/cli_test.rs`（assert_cmd 风格子进程测试）锁定：参数解析、退出码 0/1/2、`--version`（stdout 单行）、help 内容与流向、JSON 事件流 schema——这是桌面端集成的护栏。
6. **help/version 规整**：`--help` 输出改走 stdout（gumdrop 行为修正）；`-v` 预扫描 hack 清理为结构化处理。
7. **start_at 越界校验**：不在 root 之下时报错而非静默导出 0 文件。

## M3 性能（基准驱动）

1. **vault 归一化索引**：预建 NFC+lowercase 键的 HashMap，链接查找 O(链接数)，tie-break 规则在建索引时固化（承接 1.4）。
2. **嵌入解析缓存**：源文件解析结果缓存复用（embed postprocessor 仍逐次执行，保持语义）；`vault_contents` 的整表 clone 消除（`Arc` 或直接消费）。
3. **walker 并行化**：先建大 vault 基准（计时脚手架），数据支撑后再决定是否启用 ignore crate 并行遍历。
4. 每项优化附基准前后对比数据，不达预期即回退。

## 明确不做（本期范围外）

- callout 转换（仅修正 AGENTS.md 描述）。
- 块引用内容提取（TODO 登记，后续增强）。
- serde_yaml 迁移（公共 API 绑定，登记为独立后续任务）。
- 桌面端业务与任何 GUI 侧缺陷补偿。

## 桌面端需求备注（core 已预留，GUI 阶段兑现）

`--missing-section` 将放入「导出前确认选单」并持久化用户选择；进度/警告/错误经由 `--progress json` 事件流获取；启动时以 `--version` 做边车版本校验。