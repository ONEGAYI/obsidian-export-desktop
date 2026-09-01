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
  - **版本方案是 CalVer `YY.MM.PATCH`**（上游 v21.9.0 起沿用，git tag 日期与版本号逐一对得上：`22.1.0`→2022-01、`25.3.0`→2025-03、`26.8.0~26.8.5` 全在 2026-08）：前两段是发布当月的年月，是日历时间戳、不承载语义（不要按 SemVer 理解成「minor 进位表示新功能」）；第三段是该月内第几次发布，功能新增与 Bug 修复同样只递增此位。**版本不得跳月**：2026 年 8 月的下一个版本是 26.8.6，写成 26.9.0 等于把发布日期标到未来；跨月首发时月位随日历进位（9 月即 26.9.0）。`just set-version`、towncrier `--version`、tag 与 release 全按此口径。
  - fork 上 workflow 的 push/tag 事件从不触发（Actions runs 恒为 0，原因未查明），release workflow 须手动 dispatch：`gh workflow run release.yml --ref vX.Y.Z -R ONEGAYI/obsidian-export-desktop`；且 dispatch 走 tag 所指 commit 上的 workflow 定义，tag 必须指向含最新 workflow 的提交。
  - `release.yml` 的 tag 触发模式已手改为 `v*.*.*`（cargo-dist 生成的 `'**[0-9]+.[0-9]+.[0-9]+*'` 里 `+` 是字面字符，v26.8.x 从不匹配）并补 `workflow_dispatch`；因此 `dist-workspace.toml` 配了 `allow-dirty = ["ci"]` 放行 cargo-dist 对 release.yml 的漂移检查（0.28 语法为列表，布尔值会 TOML 报错）。
  - dispatch 后 macOS/Linux runner 可能长时间排队（曾 50 分钟未分配，v26.8.3 曾排队 5.5 小时后手动取消）——**不等 runner**：Windows 产物直接本地 `cargo dist build --tag vX.Y.Z` 构建后 `gh release upload`（`target/distrib/` 下连 `source.tar.gz` 与 installer 脚本都会生成），桌面安装包（`just desktop-build` 产物在 `desktop/src-tauri/target/release/bundle/{msi,nsis}/`）同法上传；tauri 生成的文件名带空格（`Obsidian Export_…`），按 v26.8.3 起惯例改名点连接（`Obsidian.Export_…`）再传；macOS/Linux 产物由 dispatch 的 workflow 排队慢慢补齐即可。
  - `make-new-release` 在 Git Bash 下会因 `just_executable()` 返回的反斜杠路径被 bash 吞掉而失败；改为手动分步执行其等价步骤：`just set-version` → `uvx towncrier==24.8.0 build --version X.Y.Z --yes` → `bash docs/generate.sh` → 提交 → `git tag vX.Y.Z`。CHANGELOG 片段必须是新式命名 `<issue>.<type>.md`（旧式 `<type>.<issue>.md` 不被识别且**静默跳过**，v26.8.4 曾手工补并入漏掉的 check-json 条目）。
  - release notes 末尾附「Downloads」资产说明段（GUI 与 CLI 是独立产物、按需下载、其余文件用途），模板见 v26.8.2；桌面与 CLI 的关系（GUI 内置边车、装 GUI 无需另装 CLI）必须写明。写入含反斜杠的路径（如 `%USERPROFILE%\.cargo\bin`）时用文件（`--notes-file`）而非 heredoc，v26.8.2 的 `\b` 曾被 shell 吃掉。
  - 注意 gh CLI 在本仓库目录下无默认 repo 时会解析到 upstream：查 fork 的 release/run 一律带 `-R ONEGAYI/obsidian-export-desktop`。
  - **（26.8.6 实践）CI 与发布的新坑**：
    - fork 的 CI 在 PR #6 前从未全绿（历史 PR 手动合并不等待），Linux 侧存量债一次性清偿后才成为基线：`cfg(not(windows))` 测试本地不可见（平台断言、`tail_expr_drop_order`）、nightly rustfmt 行为演进（imports 粒度变化需全量重排）、tarpaulin 插桩拖慢执行暴露速率类时序断言——本地 Windows 全绿不代表 CI 绿，动测试时留意平台耦合与耗时假设。
    - `docs/CHANGELOG.md` 与 `docs/CONTRIBUTING.md` 是 **git symlink（mode 120000）**：Windows checkout 把它们物化成「内容为目标路径的普通文本文件」，当普通文件编辑（哪怕只追加换行）会污染 blob，Linux 上 symlink 目标带上 `\n` 变 broken（pre-commit 的 check-symlinks 挂）。修复方式 `git hash-object -w --stdin` + `update-index --cacheinfo 120000,<hash>,<path>`；改文件前先 `git ls-files -s` 看 mode。
    - towncrier 片段正文首行**不要带 `- ` 列表前缀**（towncrier 生成时自己加，双前缀 `- - ` 需手工修 CHANGELOG）；生成后条目链接按 issue_format 指向 zoni issues，需手工替换为 fork 的 pull 链接。
- 通用行为准则、提交与发布规范以用户级 AGENTS.md 为准，此处不重复。

## 待定事项

- [x] 桌面端技术栈：**Tauri 2 + React/TypeScript**（跨平台、轻量、Obsidian 风格 UI）。UI 主题复刻 Obsidian 变量命名与色板（明暗双主题，默认暗色）；`--missing-section` 放导出前确认选单并持久化选择。
- [x] 桌面端完整选项面板：独立设置视图（`OptionsView`）暴露全部 CLI 选项，按「转换行为 / 内容过滤 / 文件与过程」分组；选项持久化于 localStorage（`obsidian-export-options`），Rust 侧 `build_args` 仅将非默认值传给边车（默认值语义始终以 CLI 为准）。
- [x] 导出后自动链接检查：已完成——check 子命令支持 `--progress json`（独立 check 事件方言、共享 schema 版本常量，契约见 `docs/sidecar-events.md` 的 check 章节）；桌面端 `start_check` 编排复用导出的 child 槽（同时仅一个边车进程），导出成功（exit 0）且开关开启时由前端自动触发；开关与检查目标（默认 vault 源，可选导出产物——两者语义不等价：死链 wikilink 导出后已塌缩为纯文本，查产物抓不到）持久化为 GUI 偏好字段（`linkCheckEnabled`/`linkCheckTarget`），不进 build_args；`LinkCheckPanel` 展示逐条报告（结构化判定本地化 + 筛选页签 + 渲染上限兜底）。
- [x] 检查更新与下载（双端）：已完成——根 crate `src/update.rs` 承载全部业务（GitHub `releases/latest` 检测、三段版本比较（CalVer 兼容，tag 不规范宁可不提示）、双意图资产挑选（cli 按编译期平台 triple 匹配 cargo-dist 产物并排除 `.sha256`；desktop 挑含 `setup` 的 NSIS 安装包）、ureq 2.12 流式下载（2.x 维护分支，MSRV 1.71；3.x 需 1.85 不可用，代理走环境变量）与原子落盘、资产名纯文件名校验；debug 构建支持 `OBSIDIAN_EXPORT_UPDATE_API_BASE` 注入本地 mock（release 无此路径））。CLI `update` 子命令（首位关键字分流，`--download`/`--output`/`--asset cli|desktop`/`--progress json`）输出第三方言（`update-result`/`download-start`/`download-progress`/`download-end`，共享 schema v1，契约见 `docs/sidecar-events.md` 的 update 章节）；**「有更新」退出码仍为 0**（GUI 按 update-result 事件判定，脚本不受扰）。桌面端 `start_update(action)` 编排复用导出 child 槽（恒传 `--asset desktop`，download 模式落盘 `%TEMP%/obsidian-export/Downloads` 并预创建+symlink 防逃逸），`run_installer` 双层路径校验后 spawn NSIS 并 `app.exit(0)` 解锁自身文件（UAC 由安装器 manifest 处理，capabilities 零改动）；前端 OptionsView 第五页「关于与更新」（`UpdatePanel` 状态外置 + 纯折叠函数，照 LinkCheckPanel 模式）与启动自动检查（`autoCheckUpdates` 偏好默认开，24h 节流存独立键 `obsidian-export-update-state`）。**已知限制**：Windows-only 桌面安装包（macOS/Linux 桌面端无资产，引导发布页）；下载与安装不校验哈希（GitHub release 资产信任模型，与手动下载等同）。
- [x] 特殊代码块渲染为图片嵌入（dot/mermaid/wavedrom/tikz）：已完成——core 新增 `src/diagrams.rs`（渲染器注册表、PATH+PATHEXT 工具解析、Windows `.cmd` 经 cmd.exe 包装执行、外部进程超时与 stderr 捕获、tikz latex+dvisvgm 双步管线、内容寻址命名 FNV-1a 与增量缓存）；`Exporter` 新配置（`diagram_renderers`/`diagram_format`/`diagram_bins`）与 run() 预扫描（按 vault 实际语言求工具集，缺失即原子化报错退出、零输出）；渲染是内建导出阶段（用户 postprocessor 之后执行，代码块替换为图片引用+SoftBreak 分隔，单块失败 warning 保留原块）；CLI 新增 `--render-diagrams`/`--diagram-format`/`--diagram-bin`；export 事件方言新增 `diagram-render` 进度事件（加法变更不 bump schema）；桌面端第六设置页「图表渲染」（导航计数 badge + 药丸复选 + svg/png 格式与回落说明 + 工具路径覆盖折叠）与运行视图渲染进度行；集成测试经 debug-only env `OBSIDIAN_EXPORT_DIAGRAM_BIN_<TOOL>` 注入跨平台 mock 渲染器（**教训**：cmd.exe 会预读管道 stdin、并行 append 共享计数文件有锁竞争——渲染源码一律走临时文件输入，每测试独立 mock 目录）。
- [ ] 桌面端后续迭代：打包分发流水线（`tauri build`，与 cargo-dist 互不干扰）。
- [x] 块引用内容提取增强：已完成——`![[note#^block-id]]` 真实定位标记块（reduce_to_block：行尾 id 标记所在段落/列表项/引用块，独立行 id 标记上方紧邻块，嵌入副本剥离 id 标记），未命中回退 `--missing-section` 三策略；同文件嵌入（`![[#Heading]]` / `![[#^id]]`）一并支持，防环靠「嵌入副本剥离 id」天然终止 + 嵌入公共入口的深度限制兜底。
- [x] 嵌入展开与 section 切分的顺序重排：已完成——parse_raw_note（raw 事件收集 + 引用规范化为五事件形态）与 expand_references（引用展开）两阶段拆分，section 切分在目标文件自己的事件流上进行后再展开内层嵌入；embed_postprocessors「看到合并嵌入后内容」的契约保持。
- [x] wikilink 格式标记与容器内标题的既有解析边界：已完成——引用文本在 raw 层以 source offset 切片保留原拼写（`__dunder__` 不再突变为 `**dunder**`，锚点/文件查找/section 匹配三处下游一致）；reduce_to_section 维护容器配对栈，blockquote（含 callout）内标题切分后事件流保持平衡。
- [x] 标题含 wikilink 的 section 引用：已完成——reduce_to_section 标题聚合识别坍缩五事件形态并按显示名聚合（label 优先，否则 `file > section` 拼接，复用 `ObsidianNoteReference::display`），`## [[mid]]` 重新可被 `![[t#mid]]` 命中，嵌入切片内 wikilink 照常展开为链接；字面单层方括号标题（`[WIP]` 类）因状态机回吐永不构成 `Text("[")+Text("[")` 相邻对而天然免疫误伤；引用文本含 `]` 的嵌套写法（`![[t#[[mid]]]]`）受 wikilink 语法限制（坍缩状态机遇 `]` 重置）仍按 missing-section 处理。
- [x] serde_yaml 迁移：已通过 Cargo package rename 迁移至 `yaml_serde` 0.10（YAML 官方组织维护的 0.9.34 直系 fork；serde_norway 等候选已停滞故未采用）。公共路径 `obsidian_export::serde_yaml` 与解析/序列化行为不变（非破坏变更）；MSRV 由 1.80 升至 1.82。
- [x] 章节锚点对齐 GitHub slug：已完成——format_anchor 委托 github-slugger crate（封装层先 trim 对齐 VS Code），全角标点无痕剔除、标点不再误产连字符、连字符不折叠不修剪；行为向量来自 2026-08 对 GitHub 网页渲染的实测（与 VS Code 官方包源码、github-slugger 三方一致），与上游 PR #373 同路线（fork 未用回 slug crate 的原因是其对中文做拼音化）。重复标题的去重后缀：check 侧已实现——`deduped_anchors` 以有状态 Slugger 按文档序生成 `-1`/`-2` 后缀（GitHub 同算法，`#dup-1` 类片段可验证）；导出写链有意无后缀（wikilink 引用恒指首个匹配标题，无后缀 slug 在 GitHub 上本就正确跳转）。
- [x] vault 链接完整性检查（core + CLI check 子命令）：已完成——`Exporter::check()`（`src/linkcheck.rs`）walk 与导出同集的文件，逐链接验证：目标存在性、**越界即断**（逃出检查根的链接即使盘上存在也判 broken，根即导出边界）、wikilink 锚点按 Obsidian 原文语义（复用 reduce_to_section/reduce_to_block 聚合）、标准 md 链接锚点按 slug 语义（format_anchor 幂等匹配）、外部 URL 跳过；引用提取复用 parse_raw_note_with_refs（其新增的源偏移旁路供行号归因），md 链接经同一 parser flavor 二次遍历，代码块/行内代码剔除语义与导出一致。CLI：`obsidian-export check SOURCE`（`--start-at`/`--hidden`/`--no-git`/`--ignore-file` 可复用；gumdrop 禁止 command 与 free 并存，子命令靠首位关键字手动分流），逐条 `{source}:{line}: {status} [{raw}]` + 汇总，退出码沿用 0/1/2 契约（有任何 broken 即 1）。
- [ ] 嵌入解析缓存与 walker 并行化：vault 索引已消除引用解析的主要瓶颈（基准 7200 文件 11.2s → 0.65s），剩余耗时以文件 IO/解析/渲染为主；两项优化待有真实大 vault 的 profile 数据支撑后再决定是否实施。

已知限制（审查登记，后续迭代评估）：

- [ ] 渲染超时的进程树击杀平台不对称：Windows 走 `taskkill /T /F` 连孙进程根治；Unix 未设进程组（避免改变 Ctrl+C 传播语义），卡死的孙进程持有 pipe 时靠 5s reader 宽限兜底（reader 线程泄漏但 `run_command` 必返，不挂起导出）。
- [ ] 嵌入展开的图表副本渲染为宿主笔记的独立资产（`<宿主stem>-<hash>` 与源笔记资产各一份，字节相同）：语义是「每份产物自包含」；代价是 `diagram-render` 的 `index` 可超过 `total`（预扫描只数源文件自身块，契约文档已声明 total 为估计值）。
- [ ] cmd.exe 包装对工具路径含成对 `%` 的形态会做变量展开（引号不保护）：罕见路径建议 `--diagram-bin` 指向 `.exe` 绕过包装（usage-advanced.md 已注明）。
- [ ] `.render-*` 临时文件在进程崩溃/被杀时残留于 `assets/`，无启动清扫（并行约束使「渲染前清目录」会误删其他 worker 的在写文件）；属垃圾累积，不影响正确性。
- [ ] 预扫描对全 vault 做二次 read+parse（启用渲染时 IO 与解析翻倍，7200 文件基准约 +0.5s）：可优化为行扫描识别 fence，待 profile 数据支撑。
- [ ] 图表 mock 集成测试整体 `#![cfg(debug_assertions)]`：`cargo test --release` 下该文件静默剔除（注入钩子 release 编译为 None，跑必假失败）。
- [ ] 引用文本含换行时 `from_str` 的正则整体不匹配（`.*?` 不跨 `\n`），理论上可触发 `expect` panic（rayon worker 中）；ref_text 状态机按事件拼接、多行输入实际罕见，存量行为多年未见报告，改动需先证明可达性。
- [ ] 同文件嵌入的嵌套解析是切片局部的：嵌入片段内的 `![[#Other]]` 只在切片内查找（Obsidian 从全文件解析），跨 section 引用在切片中查不到时按 missing-section 塌缩；块的优雅自引用终止正依赖此局部性，改为全文件解析需重新设计防环。

桌面端低风险遗留（两轮审查登记，不阻塞发布）：

- [ ] sidecar 的 stderr 仍按 chunk 分别 lossy 解码后拼接（`pump_sidecar`，与已修复的 stdout 行切分同模式）：错误消息含非 ASCII（如中文 vault 路径）且恰跨 chunk 边界会产生 U+FFFD，概率低、仅显示层瑕疵；且 stderr 累积无上限，两者宜一并处理。
- [ ] 窗口最小化/不可见时 WebView 暂停 rAF，check 事件持续入 buffer 不 flush：运行进度不刷新、buffer 短时驻留内存；数据不丢（check-exit 强制 flush 兜底），可选 setTimeout 兜底。
- [ ] check 运行中取消落入 failed 态而非 cancelled 态（无 check-end 即判失败）：文案为「未能完成 + 退出码」，语义不误导，但与导出侧的 cancelled 标志不对称。update 侧同模式：下载中取消（cancel_export 通杀共享 child 槽）经 update-exit 判为 failed。
- [ ] ureq 2.12 内部对小响应体 + 服务器截断断流的组合会 panic（exit 101，破坏边车 0/1/2 退出码契约；上游 TODO 自认）：极低概率，升级 ureq 时关注上游修复。
- [ ] OptionsView 页签未实现 roving tabindex 与方向键导航（ARIA tabs 模式的推荐增强）：当前所有 tab 均在 Tab 序列中、原生 button 可正常操作。
- [ ] 导出中切换语言存在微任务级双订阅窗口（`onSidecarEvent` 按 `[t]` 重订阅，旧注销的微任务晚于新注册的 IPC，同一事件被新旧 handler 各折叠一次——done 计数 +2、日志行重复；i18n 时代遗留，根治方案是 warningLabel 走 ref、订阅进空依赖 effect）。
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
- 事件流消费遵守 `docs/sidecar-events.md` 契约（导出 / check / update 三种事件方言）；schema 版本常量在 `desktop/src-tauri/src/events.rs` 与 CLI 的 `main.rs` 各有一份，升级时同步改。
- 设置视图为分页式（`OptionsView`：左侧导航五页「转换行为 / 内容过滤 / 文件与过程 / 链接检查 / 关于与更新」，窄窗降级横排页签）。链接检查：导出成功且开关开启时前端自动 invoke `start_check`，检查 vault 源时由 Rust 侧 `build_check_args` 转发与导出一致的非默认过滤项（walk 集对齐；tag 后处理不参与 check），检查导出产物时恒传 `--no-git` 并使 `--hidden` 与导出一致（CLI 默认值本身是过滤——产物目录在 git 仓库内会被 gitignore 静默排除成假阴性）；check 与导出共用 child 槽，`cancel_export` 通杀，`start_export` 返回实际落点供「检查产物」定位；check 流的解析/IO 错误走独立 `check-error` 通道（导出日志视图在检查期已卸载，混入 sidecar-error 会不可见）。

## 文件树（简版速览）

```
<!-- file-tree:tree:begin 由脚本渲染，禁止手改 -->
obsidian-export-desktop/
├── .agents/                # agent 技能目录
│   └── skills/ # 已部署技能目录
│       └── file-tree/ # 文件树技能（唯一数据源）
│           ├── agents/   # 技能元数据目录
│           │   └── openai.yaml # Codex 技能元数据
│           ├── scripts/  # 技能脚本目录
│           │   ├── tree_tool.py      # 文件树唯一维护脚本
│           │   └── tree_tool_test.py # tree_tool 契约测试
│           ├── SKILL.md  # 技能主入口与命令速查
│           └── tree.json # 文件树数据（唯一数据源）
├── .gitattributes          # 换行符规范
├── .github/                # GitHub 配置目录
│   ├── actions/       # 复合动作目录
│   │   ├── cargo-binstall/ # cargo 二进制安装动作
│   │   │   └── action.yaml # 复合动作：装 cargo 二进制
│   │   └── setup-ci/       # CI 环境准备动作
│   │       └── action.yaml # 复合动作：装工具链与缓存
│   ├── FUNDING.yml    # GitHub 赞助配置
│   ├── renovate.json5 # Renovate 依赖更新机器人配置
│   └── workflows/     # CI 工作流目录
│       ├── ci.yml            # CI 测试工作流
│       ├── publish-crate.yml # 可复用发布到 crates.io 工作流
│       └── release.yml       # cargo-dist 自动发布工作流
├── .gitignore              # 根忽略规则
├── .pre-commit-config.yaml # 本地与 CI 共用提交钩子
├── AGENTS.md               # 项目规则单一事实源
├── Cargo.lock              # 根 crate 依赖锁文件
├── Cargo.toml              # 主 crate 清单（lib+bin）
├── changelog.d/            # towncrier 变更片段目录
│   ├── .gitignore   # 片段目录占位忽略文件
│   └── 9.feature.md # 图表渲染功能变更片段
├── CHANGELOG.md            # 变更日志（towncrier 生成）
├── CLAUDE.md               # Claude 专属补充规则
├── cliff.toml              # git-cliff 备用变更日志配置
├── CONTRIBUTING.md         # 贡献指南（上游）
├── deny.toml               # cargo-deny 依赖审计配置
├── desktop/                # Tauri 2 桌面端
│   ├── .gitignore          # 前端目录 Git 忽略规则
│   ├── .vscode/            # VS Code 配置目录
│   │   └── extensions.json # VS Code 推荐扩展列表
│   ├── components.json     # shadcn/ui CLI 配置
│   ├── index.html          # Vite HTML 壳页面
│   ├── package.json        # 前端包清单与脚本定义
│   ├── pnpm-lock.yaml      # pnpm 依赖锁文件
│   ├── pnpm-workspace.yaml # pnpm 构建许可白名单
│   ├── public/             # Vite 静态资源目录
│   │   ├── tauri.svg # Tauri 官方 logo 图标
│   │   └── vite.svg  # Vite logo（favicon）
│   ├── README.md           # Tauri 模板遗留说明
│   ├── scripts/            # 前端辅助脚本目录
│   │   └── sync-sidecar.mjs # 构建 CLI 并同步为 Tauri 边车
│   ├── src/                # 桌面端前端源码（React/TS）
│   │   ├── App.tsx       # 应用根组件，串联三阶段导出流程
│   │   ├── components/   # 视图组件目录
│   │   │   ├── ExportDialog.tsx     # 导出前确认对话框
│   │   │   ├── ExportResultView.tsx # 导出结果汇总卡片
│   │   │   ├── ExportRunView.tsx    # 导出进行中的进度与日志视图
│   │   │   ├── LinkCheckPanel.tsx   # 链接检查面板与状态折叠逻辑
│   │   │   ├── OptionsView.tsx      # 分页式转换选项设置面板
│   │   │   ├── PathPicker.tsx       # 目录路径输入加浏览选择器
│   │   │   ├── TagInput.tsx         # 多标签芯片输入编辑器
│   │   │   ├── ui/                  # 手搭 shadcn 基础组件层
│   │   │   │   ├── button.tsx        # 基础按钮组件（cva 变体）
│   │   │   │   ├── card.tsx          # 卡片及其分区容器组件
│   │   │   │   ├── checkbox.tsx      # 手写复选框，支持半选态
│   │   │   │   ├── dialog.tsx        # 基于 radix 的模态对话框
│   │   │   │   ├── dropdown-menu.tsx # 基于 radix 的下拉菜单
│   │   │   │   ├── input.tsx         # 单行文本输入框组件
│   │   │   │   ├── label.tsx         # 基于 radix 的表单标签
│   │   │   │   ├── progress.tsx      # 基于 radix 的进度条
│   │   │   │   ├── radio-group.tsx   # 基于 radix 的单选组
│   │   │   │   └── switch.tsx        # 基于 radix 的开关
│   │   │   └── UpdatePanel.tsx      # 关于与更新页（状态外置）
│   │   ├── i18n/         # 界面国际化目录
│   │   │   ├── en.ts     # 英文字典，键型受 Dict 约束
│   │   │   ├── index.tsx # i18n 上下文与语言偏好分发
│   │   │   └── zh.ts     # 中文字典并定义 Dict 类型基准
│   │   ├── index.css     # Obsidian 风格主题变量与全局样式
│   │   ├── lib/          # 前端工具与封装层目录
│   │   │   ├── options.ts # 导出选项类型、校验与摘要
│   │   │   ├── sidecar.ts # Tauri 命令调用与事件封装层
│   │   │   ├── theme.ts   # 主题偏好 Hook，支持跟随系统
│   │   │   └── utils.ts   # cn 类名合并工具函数
│   │   ├── main.tsx      # React 入口（含 i18n）
│   │   └── vite-env.d.ts # Vite 客户端类型引用
│   ├── src-tauri/          # Tauri Rust 后端
│   │   ├── .cargo/         # cargo 配置目录
│   │   │   └── config.toml # MSRV 感知的依赖解析配置
│   │   ├── .gitignore      # 后端构建产物忽略规则
│   │   ├── binaries/       # 边车二进制落位目录（不入库）
│   │   ├── build.rs        # 标准 tauri-build 构建脚本
│   │   ├── capabilities/   # Tauri 权限能力声明目录
│   │   │   └── default.json # 主窗口权限能力声明
│   │   ├── Cargo.lock      # 桌面 workspace 依赖锁文件
│   │   ├── Cargo.toml      # 桌面 crate 清单（独立 ws）
│   │   ├── icons/          # Tauri 全平台应用图标集
│   │   ├── src/            # 后端源码（4 个模块）
│   │   │   ├── events.rs  # JSON Lines 事件解析
│   │   │   ├── lib.rs     # Tauri Builder 装配
│   │   │   ├── main.rs    # Tauri 应用二进制入口
│   │   │   └── sidecar.rs # CLI 边车进程编排核心
│   │   └── tauri.conf.json # Tauri 应用与打包配置
│   ├── tsconfig.json       # 前端 TS 主编译配置
│   ├── tsconfig.node.json  # Vite 配置文件的 TS 子项目
│   └── vite.config.ts      # Vite 配置，适配 Tauri 开发
├── dist-workspace.toml     # cargo-dist 发布工作区配置
├── docs/                   # 项目文档（mdBook+fork）
│   ├── .obsidian/           # Obsidian 编辑器工作区配置
│   ├── _combined.md         # README 生成的章节嵌入清单
│   ├── _edit-warning.md     # 勿直接编辑 README 的警告块
│   ├── BUILD.md             # 中文构建指南（CLI 与桌面端）
│   ├── CHANGELOG.md         # 指向根变更日志的指针文件
│   ├── changes.md           # 更新日志引导页
│   ├── contribute.md        # 贡献引导页
│   ├── CONTRIBUTING.md      # 指向根贡献指南的指针文件
│   ├── desktop.md           # Tauri 桌面端功能介绍
│   ├── generate.sh          # 由 docs 生成 README 的脚本
│   ├── installation.md      # 安装与升级指南
│   ├── intro.md             # 项目简介与核心特性列表
│   ├── license.md           # 许可证说明
│   ├── Release-checklist.md # 发布流程检查清单
│   ├── sidecar-events.md    # 边车 JSON 事件流契约文档
│   ├── usage-advanced.md    # CLI 高级选项与技巧
│   ├── usage-basic.md       # CLI 基本用法说明
│   └── usage-library.md     # Rust 库使用指引
├── Justfile                # 核心任务入口（桌面端+发布）
├── LICENSE                 # 上游许可证全文
├── README.md               # 项目自述（generate.sh 产物）
├── rust-toolchain.toml     # 固定 Rust 工具链版本
├── rustfmt.toml            # 代码格式化规则（需 nightly）
├── src/                    # Rust 源码（CLI 与库）
│   ├── context.rs        # 笔记解析上下文与嵌套追踪
│   ├── diagrams.rs       # 图表渲染：渲染器注册表与外部工具编排
│   ├── frontmatter.rs    # frontmatter 类型与序列化
│   ├── lib.rs            # 库核心：Exporter 导出引擎
│   ├── linkcheck.rs      # 链接完整性检查（check 后端）
│   ├── main.rs           # CLI 二进制入口与参数解析
│   ├── postprocessors.rs # 官方内置后处理器集合
│   ├── references.rs     # Obsidian 引用解析器与状态机
│   ├── update.rs         # 更新检测与下载（双端共享）
│   └── walker.rs         # 库文件遍历与忽略规则
├── tests/                  # 集成测试目录
│   ├── cli_test.rs            # CLI 契约测试（供桌面端依赖）
│   ├── diagrams_cli_test.rs   # 图表渲染 CLI 集成测试
│   ├── export_test.rs         # 库级导出功能集成测试
│   ├── postprocessors_test.rs # 后处理器行为测试
│   └── testdata/              # 测试 vault fixtures
│       ├── expected/ # 黄金输出树（8 个场景）
│       │   ├── filter-by-tags/                      # 标签过滤黄金输出树
│       │   ├── infinite-recursion/                  # 循环嵌入降级输出树
│       │   ├── main-samples/                        # 主样例黄金输出树
│       │   ├── non-ascii/                           # 非 ASCII 黄金输出树
│       │   ├── postprocessors/                      # 后处理器黄金输出树
│       │   ├── same-filename-different-directories/ # 同名消解黄金输出树
│       │   ├── single-file/                         # 单文件黄金输出树
│       │   └── start-at/                            # start-at 黄金输出树
│       └── input/    # 测试输入 vault（20 个场景）
│           ├── block-refs/                          # 块引用嵌入定位数据
│           ├── block-refs-edge/                     # 块引用边界情况数据
│           ├── block-refs-self-loop/                # 块自引用循环数据
│           ├── chinese-anchor/                      # 中文标题锚点数据
│           ├── diagrams/                            # 四语言图表块样例 vault
│           ├── diagrams-alias/                      # 语言别名图表块 vault
│           ├── diagrams-fail/                       # 渲染失败保留场景 vault
│           ├── diagrams-png/                        # png 回落场景 vault
│           ├── embed-order/                         # 嵌入切片先于展开数据
│           ├── escaped-pipe-refs/                   # 表格内转义竖线 wikilink 数据
│           ├── filter-by-tags/                      # 标签过滤数据
│           ├── formatting-refs/                     # 带格式标记 wikilink 数据
│           ├── heading-wikilink/                    # 标题含 wikilink 嵌入数据
│           ├── image-size/                          # 图片尺寸语法数据
│           ├── infinite-recursion/                  # 循环嵌入测试数据
│           ├── main-samples/                        # 主样例库（多测试复用）
│           ├── missing-sections/                    # 缺失章节策略数据
│           ├── mixed-health/                        # 混合健康度笔记数据
│           ├── non-ascii/                           # 非 ASCII 文件名数据
│           ├── numbered-section/                    # 编号章节锚点数据
│           ├── postprocessors/                      # 后处理器测试数据
│           ├── relative-references/                 # 相对路径引用数据
│           ├── same-filename-different-directories/ # 同名文件歧义消解数据
│           ├── section-variants/                    # 章节匹配变体数据
│           ├── single-file/                         # 单文件导出数据
│           └── start-at/                            # start-at 子路径数据
└── towncrier.toml          # towncrier 变更日志配置
<!-- file-tree:tree:end -->
```

## 文件树标签词表

<!-- file-tree:tags:begin 由脚本渲染，禁止手改 -->
| 标签 | 说明 |
| --- | --- |
| `ci` | CI/CD 与发布流水线 |
| `config` | 构建/工具链/项目配置 |
| `docs` | 文档 |
| `fixture` | 测试 vault 数据（场景目录粗粒度收录） |
| `frontend` | 桌面端前端 TS/TSX/CSS |
| `generated` | 生成产物，禁止手改 |
| `i18n` | 界面国际化 |
| `meta` | 项目规则与技能自身 |
| `rust` | Rust 源码（根 crate 或 src-tauri） |
| `sidecar` | 边车（CLI 进程）编排与契约 |
| `tauri` | Tauri 桌面端（Rust 后端） |
| `test` | 测试代码 |
<!-- file-tree:tags:end -->
