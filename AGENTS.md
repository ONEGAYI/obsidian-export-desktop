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
- [x] 导出后自动链接检查：已完成——check 子命令支持 `--progress json`（独立 check 事件方言、共享 schema 版本常量，契约见 `docs/sidecar-events.md` 的 check 章节）；桌面端 `start_check` 编排复用导出的 child 槽（同时仅一个边车进程），导出成功（exit 0）且开关开启时由前端自动触发；开关与检查目标（默认 vault 源，可选导出产物——两者语义不等价：死链 wikilink 导出后已塌缩为纯文本，查产物抓不到）持久化为 GUI 偏好字段（`linkCheckEnabled`/`linkCheckTarget`），不进 build_args；`LinkCheckPanel` 展示逐条报告（结构化判定本地化 + 筛选页签 + 渲染上限兜底）。
- [ ] 桌面端后续迭代：打包分发流水线（`tauri build`，与 cargo-dist 互不干扰）、自动更新。
- [x] 块引用内容提取增强：已完成——`![[note#^block-id]]` 真实定位标记块（reduce_to_block：行尾 id 标记所在段落/列表项/引用块，独立行 id 标记上方紧邻块，嵌入副本剥离 id 标记），未命中回退 `--missing-section` 三策略；同文件嵌入（`![[#Heading]]` / `![[#^id]]`）一并支持，防环靠「嵌入副本剥离 id」天然终止 + 嵌入公共入口的深度限制兜底。
- [x] 嵌入展开与 section 切分的顺序重排：已完成——parse_raw_note（raw 事件收集 + 引用规范化为五事件形态）与 expand_references（引用展开）两阶段拆分，section 切分在目标文件自己的事件流上进行后再展开内层嵌入；embed_postprocessors「看到合并嵌入后内容」的契约保持。
- [x] wikilink 格式标记与容器内标题的既有解析边界：已完成——引用文本在 raw 层以 source offset 切片保留原拼写（`__dunder__` 不再突变为 `**dunder**`，锚点/文件查找/section 匹配三处下游一致）；reduce_to_section 维护容器配对栈，blockquote（含 callout）内标题切分后事件流保持平衡。
- [x] 标题含 wikilink 的 section 引用：已完成——reduce_to_section 标题聚合识别坍缩五事件形态并按显示名聚合（label 优先，否则 `file > section` 拼接，复用 `ObsidianNoteReference::display`），`## [[mid]]` 重新可被 `![[t#mid]]` 命中，嵌入切片内 wikilink 照常展开为链接；字面单层方括号标题（`[WIP]` 类）因状态机回吐永不构成 `Text("[")+Text("[")` 相邻对而天然免疫误伤；引用文本含 `]` 的嵌套写法（`![[t#[[mid]]]]`）受 wikilink 语法限制（坍缩状态机遇 `]` 重置）仍按 missing-section 处理。
- [x] serde_yaml 迁移：已通过 Cargo package rename 迁移至 `yaml_serde` 0.10（YAML 官方组织维护的 0.9.34 直系 fork；serde_norway 等候选已停滞故未采用）。公共路径 `obsidian_export::serde_yaml` 与解析/序列化行为不变（非破坏变更）；MSRV 由 1.80 升至 1.82。
- [x] 章节锚点对齐 GitHub slug：已完成——format_anchor 委托 github-slugger crate（封装层先 trim 对齐 VS Code），全角标点无痕剔除、标点不再误产连字符、连字符不折叠不修剪；行为向量来自 2026-08 对 GitHub 网页渲染的实测（与 VS Code 官方包源码、github-slugger 三方一致），与上游 PR #373 同路线（fork 未用回 slug crate 的原因是其对中文做拼音化）。已知限制：同文档重复标题的 GitHub `-1` 去重后缀需要文档级状态，未实现。
- [x] vault 链接完整性检查（core + CLI check 子命令）：已完成——`Exporter::check()`（`src/linkcheck.rs`）walk 与导出同集的文件，逐链接验证：目标存在性、**越界即断**（逃出检查根的链接即使盘上存在也判 broken，根即导出边界）、wikilink 锚点按 Obsidian 原文语义（复用 reduce_to_section/reduce_to_block 聚合）、标准 md 链接锚点按 slug 语义（format_anchor 幂等匹配）、外部 URL 跳过；引用提取复用 parse_raw_note_with_refs（其新增的源偏移旁路供行号归因），md 链接经同一 parser flavor 二次遍历，代码块/行内代码剔除语义与导出一致。CLI：`obsidian-export check SOURCE`（`--start-at`/`--hidden`/`--no-git`/`--ignore-file` 可复用；gumdrop 禁止 command 与 free 并存，子命令靠首位关键字手动分流），逐条 `{source}:{line}: {status} [{raw}]` + 汇总，退出码沿用 0/1/2 契约（有任何 broken 即 1）。
- [ ] 嵌入解析缓存与 walker 并行化：vault 索引已消除引用解析的主要瓶颈（基准 7200 文件 11.2s → 0.65s），剩余耗时以文件 IO/解析/渲染为主；两项优化待有真实大 vault 的 profile 数据支撑后再决定是否实施。

已知限制（审查登记，后续迭代评估）：

- [ ] 同文件嵌入的嵌套解析是切片局部的：嵌入片段内的 `![[#Other]]` 只在切片内查找（Obsidian 从全文件解析），跨 section 引用在切片中查不到时按 missing-section 塌缩；块的优雅自引用终止正依赖此局部性，改为全文件解析需重新设计防环。

桌面端低风险遗留（两轮审查登记，不阻塞发布）：

- [ ] sidecar 的 stderr 仍按 chunk 分别 lossy 解码后拼接（`pump_sidecar`，与已修复的 stdout 行切分同模式）：错误消息含非 ASCII（如中文 vault 路径）且恰跨 chunk 边界会产生 U+FFFD，概率低、仅显示层瑕疵；且 stderr 累积无上限，两者宜一并处理。
- [ ] 窗口最小化/不可见时 WebView 暂停 rAF，check 事件持续入 buffer 不 flush：运行进度不刷新、buffer 短时驻留内存；数据不丢（check-exit 强制 flush 兜底），可选 setTimeout 兜底。
- [ ] check 运行中取消落入 failed 态而非 cancelled 态（无 check-end 即判失败）：文案为「未能完成 + 退出码」，语义不误导，但与导出侧的 cancelled 标志不对称。
- [ ] OptionsView 页签未实现 roving tabindex 与方向键导航（ARIA tabs 模式的推荐增强）：当前所有 tab 均在 Tab 序列中、原生 button 可正常操作。
- [ ] i18n 的 `fmt` 占位符（`{name}`）与字典字符串之间无编译期校验：`Widen` 宽化只锁键结构不锁占位符，拼错时运行时输出原文模板。

## 修复路线（已批准）

按「正确性 → 稳健性 → 性能」三阶段推进（M0 环境基线 → M1 正确性 → M2 稳健性 → M3 性能），彻底完善前不动桌面端业务。GUI 极简原则：缺陷修在 core/CLI 层，桌面端只做展示。三阶段已完成并经两轮子代理审查-修复-复审闭环。

## 桌面端开发（Tauri）

- 常用命令：`just desktop-sync-sidecar`（构建 CLI 并复制到 `desktop/src-tauri/binaries/`，**改动 CLI 后必须重跑**）、`just desktop-dev`、`just desktop-build`、`just desktop-test`、`just clean <target|desktop|sidecar|all>`（清理中间产物与依赖，范围可选）。
- 完整构建说明（含 Windows 坑与图标替换）见 [docs/BUILD.md](docs/BUILD.md)（中文，面向人类读者）。
- `desktop/src-tauri` 是独立 cargo workspace（自持 `[workspace]`），不影响根 crate 与 cargo-dist；`.cargo/config.toml` 启用 MSRV 感知解析（工具链锁 1.87）。
- 前端：Tailwind v4 + 手搭 shadcn 层（shadcn CLI 与当前 Node 生态冲突，组件手写在 `src/components/ui/`；CLI 修复后可迁移）。主题三态（light/dark/system，`src/lib/theme.ts`）；用户偏好存 localStorage：路径记忆、「保留根文件夹」（`obsidian-export-*` 逐项键）与转换选项（`obsidian-export-options` 单键 JSON，见 `src/lib/options.ts`）。
- i18n：界面文案抽离为字典（`src/i18n/`，zh 为结构基准、`Widen` 宽化出 `Dict` 类型锁两份字典键一致），运行时经项目首个 React Context（`I18nProvider`）分发；语言三态 zh/en/system（跟随系统按 `navigator.languages` 是否含 zh 前缀判定），偏好存 `obsidian-export-language`，生效语言同步 `document.documentElement.lang`；标题栏下拉（`LanguageMenu`，radix dropdown-menu）三态互转。Rust/CLI 侧英文技术错误原文透传，不进字典。
- 版本号统一由 `just set-version X.Y.Z` 控制：一次对齐六处——根 crate（`Cargo.toml` + `Cargo.lock`）与桌面端三处（`desktop/package.json`、`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/Cargo.toml` + 其 `Cargo.lock`），避免安装包文件名与 release 版本错位（26.8.2 起对齐）。`make-new-release` 已接入该目标。依赖 cargo-edit（仓库工具链锁 1.87 而 cargo-edit 0.13.13 要求 1.92，**须在仓库外目录用 stable 工具链安装 0.13.10**：`rustup run stable cargo install cargo-edit --version 0.13.10 --locked`）；`cargo set-version` 拒绝降级（发布防呆，误 bump 的还原属手动操作）。桌面端 lock 由脚本 sed 直接修补——桌面 workspace 的 build script 依赖已同步的 sidecar 二进制，`cargo check` 在 clean 后不可用。
- 事件流消费遵守 `docs/sidecar-events.md` 契约（导出与 check 两种事件方言）；schema 版本常量在 `desktop/src-tauri/src/events.rs` 与 CLI 的 `main.rs` 各有一份，升级时同步改。
- 设置视图为分页式（`OptionsView`：左侧导航四页「转换行为 / 内容过滤 / 文件与过程 / 链接检查」，窄窗降级横排页签）。链接检查：导出成功且开关开启时前端自动 invoke `start_check`，检查 vault 源时由 Rust 侧 `build_check_args` 转发与导出一致的非默认过滤项（walk 集对齐；tag 后处理不参与 check），检查导出产物时恒传 `--no-git` 并使 `--hidden` 与导出一致（CLI 默认值本身是过滤——产物目录在 git 仓库内会被 gitignore 静默排除成假阴性）；check 与导出共用 child 槽，`cancel_export` 通杀，`start_export` 返回实际落点供「检查产物」定位；check 流的解析/IO 错误走独立 `check-error` 通道（导出日志视图在检查期已卸载，混入 sidecar-error 会不可见）。

## 文件树

```text
obsidian-export/
├── src/                 # Rust 源码（CLI 与库）
│   ├── main.rs          # CLI 入口（bin）：参数解析、JSON 事件流输出与错误报告
│   ├── lib.rs           # 库入口：导出流程、事件回调、错误聚合与 VaultIndex
│   ├── context.rs       # 导出上下文与配置
│   ├── frontmatter.rs   # frontmatter 的解析与剥离
│   ├── linkcheck.rs     # 链接完整性检查（Exporter::check）：存在性/越界/锚点有效性 + 逐条报告
│   ├── postprocessors.rs# 后处理器：对导出结果再加工
│   ├── references.rs    # 引用解析：wikilink、嵌入等链接形式
│   └── walker.rs        # vault 的递归遍历
├── tests/               # 集成测试：export_test（导出行为）、cli_test（CLI 契约）、postprocessors_test
├── tests/testdata/      # 测试 vault fixtures（section-variants、image-size 等按场景分组）
├── desktop/             # Tauri 2 桌面端（前端 React/TS + src-tauri Rust 后端，独立 workspace）
│   ├── src/components/  # 视图组件：OptionsView（分页式设置）、ExportRunView/ExportResultView（进度与结果）、LinkCheckPanel（链接检查报告）等
│   └── src-tauri/
│       ├── src/events.rs  # sidecar JSON Lines 事件解析（schema v1，导出与 check 两种方言，单元测试锁定）
│       ├── src/sidecar.rs # 边车编排：版本握手 / spawn / 事件转发（共享 pump）/ 取消 / 导出落点解析 / start_check 链接检查 / 参数构建（build_args·build_check_args 仅传非默认值）
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
