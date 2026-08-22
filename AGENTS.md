# AGENTS.md

## 项目定位

本项目基于上游开源项目 [zoni/obsidian-export](https://github.com/zoni/obsidian-export)（Rust 实现的 Obsidian vault → 通用 Markdown 导出工具）改造而来，目标是为它补齐一个**桌面图形界面**。

## 改造目的：桌面端 + 边车架构

整体采用「桌面端 + 边车（sidecar）」双进程架构。

> 边车（sidecar）：随主程序一同运行、承担具体业务的辅助进程。本项目中边车就是 obsidian-export 的 CLI。

| 部分 | 形态 | 职责 |
|------|------|------|
| 桌面端 | Tauri 2 + React/TS（`desktop/`，脚手架已就绪） | 一切 GUI 事务：界面渲染、文件与文件夹选择、转换选项配置、进度与结果展示；边车进程编排（spawn/事件流消费/取消）在 Tauri 的 Rust 后端 |
| 边车 | 已有 CLI | 实际业务：遍历 vault，将 Obsidian 方言语法（wikilink 双链、嵌入等）转换为通用 Markdown。**callout（`> [!NOTE]` 等）不在转换范围内，导出时原样保留**，通用 Markdown 渲染器会将其显示为字面文本 |

职责边界是硬约束：

- 桌面端**不得**实现任何转换逻辑，一律通过启动边车进程、传递参数并读取输出完成。
- 边车**保持可独立使用**：在终端单独运行时功能完整，桌面端只是它的一种调用方式。

桌面端调用边车的事件流契约（`--progress json` 的 JSON Lines 协议、终止协议、退出码、路径约定）见 [docs/sidecar-events.md](docs/sidecar-events.md)。

## 仓库约定

- 远端策略已定：fork 上游 zoni/obsidian-export 并改名为 [ONEGAYI/obsidian-export-desktop](https://github.com/ONEGAYI/obsidian-export-desktop)。`origin` 指向 fork（SSH，桌面端开发主远端），`upstream` 指向上游（同步用）。
- `desktop` 分支直接推送 fork；面向上游的 issue/commit/PR 使用英文（贡献上游时走 PR），本地自有提交与文档使用中文。
- **发布（fork 现状，26.8.2 起登记）**：
  - fork 上 workflow 的 push/tag 事件从不触发（Actions runs 恒为 0，原因未查明），release workflow 须手动 dispatch：`gh workflow run release.yml --ref vX.Y.Z -R ONEGAYI/obsidian-export-desktop`；且 dispatch 走 tag 所指 commit 上的 workflow 定义，tag 必须指向含最新 workflow 的提交。
  - `release.yml` 的 tag 触发模式已手改为 `v*.*.*`（cargo-dist 生成的 `'**[0-9]+.[0-9]+.[0-9]+*'` 里 `+` 是字面字符，v26.8.x 从不匹配）并补 `workflow_dispatch`；因此 `dist-workspace.toml` 配了 `allow-dirty = ["ci"]` 放行 cargo-dist 对 release.yml 的漂移检查（0.28 语法为列表，布尔值会 TOML 报错）。
  - dispatch 后 macOS/Linux runner 可能长时间排队（曾 50 分钟未分配）；Windows 产物可本地 `dist build --tag vX.Y.Z` 补齐后 `gh release upload`，桌面安装包（`just desktop-build` 产物在 `desktop/src-tauri/target/release/bundle/{msi,nsis}/`）同法上传。
  - release notes 末尾附「Downloads」资产说明段（GUI 与 CLI 是独立产物、按需下载、其余文件用途），模板见 v26.8.2；桌面与 CLI 的关系（GUI 内置边车、装 GUI 无需另装 CLI）必须写明。
  - 注意 gh CLI 在本仓库目录下无默认 repo 时会解析到 upstream：查 fork 的 release/run 一律带 `-R ONEGAYI/obsidian-export-desktop`。
- 通用行为准则、提交与发布规范以用户级 AGENTS.md 为准，此处不重复。

## 待定事项

- [x] 桌面端技术栈：**Tauri 2 + React/TypeScript**（跨平台、轻量、Obsidian 风格 UI）。UI 主题复刻 Obsidian 变量命名与色板（明暗双主题，默认暗色）；`--missing-section` 放导出前确认选单并持久化选择。
- [x] 桌面端完整选项面板：独立设置视图（`OptionsView`）暴露全部 CLI 选项，按「转换行为 / 内容过滤 / 文件与过程」分组；选项持久化于 localStorage（`obsidian-export-options`），Rust 侧 `build_args` 仅将非默认值传给边车（默认值语义始终以 CLI 为准）。
- [ ] 桌面端后续迭代：打包分发流水线（`tauri build`，与 cargo-dist 互不干扰）、自动更新。
- [x] 块引用内容提取增强：已完成——`![[note#^block-id]]` 真实定位标记块（reduce_to_block：行尾 id 标记所在段落/列表项/引用块，独立行 id 标记上方紧邻块，嵌入副本剥离 id 标记），未命中回退 `--missing-section` 三策略；同文件嵌入（`![[#Heading]]` / `![[#^id]]`）一并支持，防环靠「嵌入副本剥离 id」天然终止 + 嵌入公共入口的深度限制兜底。
- [x] 嵌入展开与 section 切分的顺序重排：已完成——parse_raw_note（raw 事件收集 + 引用规范化为五事件形态）与 expand_references（引用展开）两阶段拆分，section 切分在目标文件自己的事件流上进行后再展开内层嵌入；embed_postprocessors「看到合并嵌入后内容」的契约保持。
- [x] wikilink 格式标记与容器内标题的既有解析边界：已完成——引用文本在 raw 层以 source offset 切片保留原拼写（`__dunder__` 不再突变为 `**dunder**`，锚点/文件查找/section 匹配三处下游一致）；reduce_to_section 维护容器配对栈，blockquote（含 callout）内标题切分后事件流保持平衡。
- [x] 标题含 wikilink 的 section 引用：已完成——reduce_to_section 标题聚合识别坍缩五事件形态并按显示名聚合（label 优先，否则 `file > section` 拼接，复用 `ObsidianNoteReference::display`），`## [[mid]]` 重新可被 `![[t#mid]]` 命中，嵌入切片内 wikilink 照常展开为链接；字面单层方括号标题（`[WIP]` 类）因状态机回吐永不构成 `Text("[")+Text("[")` 相邻对而天然免疫误伤；引用文本含 `]` 的嵌套写法（`![[t#[[mid]]]]`）受 wikilink 语法限制（坍缩状态机遇 `]` 重置）仍按 missing-section 处理。
- [x] serde_yaml 迁移：已通过 Cargo package rename 迁移至 `yaml_serde` 0.10（YAML 官方组织维护的 0.9.34 直系 fork；serde_norway 等候选已停滞故未采用）。公共路径 `obsidian_export::serde_yaml` 与解析/序列化行为不变（非破坏变更）；MSRV 由 1.80 升至 1.82。
- [ ] 嵌入解析缓存与 walker 并行化：vault 索引已消除引用解析的主要瓶颈（基准 7200 文件 11.2s → 0.65s），剩余耗时以文件 IO/解析/渲染为主；两项优化待有真实大 vault 的 profile 数据支撑后再决定是否实施。

已知限制（审查登记，后续迭代评估）：

- [ ] 同文件嵌入的嵌套解析是切片局部的：嵌入片段内的 `![[#Other]]` 只在切片内查找（Obsidian 从全文件解析），跨 section 引用在切片中查不到时按 missing-section 塌缩；块的优雅自引用终止正依赖此局部性，改为全文件解析需重新设计防环。

## 修复路线（已批准）

按「正确性 → 稳健性 → 性能」三阶段推进（M0 环境基线 → M1 正确性 → M2 稳健性 → M3 性能），彻底完善前不动桌面端业务。GUI 极简原则：缺陷修在 core/CLI 层，桌面端只做展示。三阶段已完成并经两轮子代理审查-修复-复审闭环。

## 桌面端开发（Tauri）

- 常用命令：`just desktop-sync-sidecar`（构建 CLI 并复制到 `desktop/src-tauri/binaries/`，**改动 CLI 后必须重跑**）、`just desktop-dev`、`just desktop-build`、`just desktop-test`、`just clean <target|desktop|sidecar|all>`（清理中间产物与依赖，范围可选）。
- 完整构建说明（含 Windows 坑与图标替换）见 [docs/BUILD.md](docs/BUILD.md)（中文，面向人类读者）。
- `desktop/src-tauri` 是独立 cargo workspace（自持 `[workspace]`），不影响根 crate 与 cargo-dist；`.cargo/config.toml` 启用 MSRV 感知解析（工具链锁 1.87）。
- 前端：Tailwind v4 + 手搭 shadcn 层（shadcn CLI 与当前 Node 生态冲突，组件手写在 `src/components/ui/`；CLI 修复后可迁移）。主题三态（light/dark/system，`src/lib/theme.ts`）；用户偏好存 localStorage：路径记忆、「保留根文件夹」（`obsidian-export-*` 逐项键）与转换选项（`obsidian-export-options` 单键 JSON，见 `src/lib/options.ts`）。
- i18n：界面文案抽离为字典（`src/i18n/`，zh 为结构基准、`Widen` 宽化出 `Dict` 类型锁两份字典键一致），运行时经项目首个 React Context（`I18nProvider`）分发；语言三态 zh/en/system（跟随系统按 `navigator.languages` 是否含 zh 前缀判定），偏好存 `obsidian-export-language`，生效语言同步 `document.documentElement.lang`；标题栏下拉（`LanguageMenu`，radix dropdown-menu）三态互转。Rust/CLI 侧英文技术错误原文透传，不进字典。
- 桌面端版本号三处（`desktop/package.json`、`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/Cargo.toml`）随 CLI 版本同步（26.8.2 起对齐），发布时一并 bump，避免安装包文件名与 release 版本错位。
- 事件流消费遵守 `docs/sidecar-events.md` 契约；schema 版本常量在 `desktop/src-tauri/src/events.rs` 与 CLI 的 `main.rs` 各有一份，升级时同步改。

## 文件树

```text
obsidian-export/
├── src/                 # Rust 源码（CLI 与库）
│   ├── main.rs          # CLI 入口（bin）：参数解析、JSON 事件流输出与错误报告
│   ├── lib.rs           # 库入口：导出流程、事件回调、错误聚合与 VaultIndex
│   ├── context.rs       # 导出上下文与配置
│   ├── frontmatter.rs   # frontmatter 的解析与剥离
│   ├── postprocessors.rs# 后处理器：对导出结果再加工
│   ├── references.rs    # 引用解析：wikilink、嵌入等链接形式
│   └── walker.rs        # vault 的递归遍历
├── tests/               # 集成测试：export_test（导出行为）、cli_test（CLI 契约）、postprocessors_test
├── tests/testdata/      # 测试 vault fixtures（section-variants、image-size 等按场景分组）
├── desktop/             # Tauri 2 桌面端（前端 React/TS + src-tauri Rust 后端，独立 workspace）
│   └── src-tauri/
│       ├── src/events.rs  # sidecar JSON Lines 事件解析（schema v1，单元测试锁定）
│       ├── src/sidecar.rs # 边车编排：版本握手 / spawn / 事件转发 / 取消 / 导出落点解析 / CLI 选项参数构建（build_args 仅传非默认值）
│       ├── icons/         # Tauri 全平台应用图标及 1024px 透明主图
│       └── binaries/      # sidecar 二进制（just 同步，不入库）
├── docs/                # 项目文档：sidecar-events.md（事件契约）、BUILD.md（构建指南）、desktop.md（README 的桌面端章节）
├── changelog.d/         # towncrier 的 changelog 片段（发布时收集进 CHANGELOG.md）
├── .github/             # CI 工作流
├── AGENTS.md            # 本文件：项目规则单一事实源
├── CLAUDE.md            # Claude 专属补充规则（@AGENTS.md 导入）
├── Cargo.toml           # crate 清单
├── .gitattributes       # 换行符规范：全库 LF 检出，二进制资源标记
├── Justfile             # 常用任务命令（just）
├── dist-workspace.toml  # cargo-dist 发布打包配置
├── cliff.toml           # git-cliff 生成 changelog 的配置
├── towncrier.toml       # changelog 片段管理配置
├── deny.toml            # cargo-deny 依赖许可与安全审计
└── rust-toolchain.toml  # 固定的 Rust 工具链版本
```

> 文件树摘要基于文件名与 README 归纳，随改造推进持续维护；新增桌面端代码后应在此登记。
