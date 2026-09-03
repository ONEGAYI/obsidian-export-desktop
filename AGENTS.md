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
  - ~~fork 上 workflow 的 push/tag 事件从不触发~~（**26.9.0 实况更新**：tag 推送已能自动触发 release workflow——`v*.*.*` 触发模式修正后生效，v26.8.x 期间「从不触发」应是当时 tag 模式尚不匹配所致）。**tag 自动触发与手动 dispatch 会叠加重复 run**：v26.9.0 发布时二者并发，取消了排队中的 tag 触发 run 保留 dispatch 的。日常发布打完 tag 后观察 `gh run list --workflow=release.yml`：tag 已触发即无需 dispatch；确需手动 dispatch 用 `gh workflow run release.yml --ref vX.Y.Z -R ONEGAYI/obsidian-export-desktop`。dispatch 走 tag 所指 commit 上的 workflow 定义，tag 必须指向含最新 workflow 的提交。
  - `release.yml` 的 tag 触发模式已手改为 `v*.*.*`（cargo-dist 生成的 `'**[0-9]+.[0-9]+.[0-9]+*'` 里 `+` 是字面字符，v26.8.x 从不匹配）并补 `workflow_dispatch`；因此 `dist-workspace.toml` 配了 `allow-dirty = ["ci"]` 放行 cargo-dist 对 release.yml 的漂移检查（0.28 语法为列表，布尔值会 TOML 报错）。
  - dispatch 后 macOS/Linux runner 可能长时间排队（曾 50 分钟未分配，v26.8.3 曾排队 5.5 小时后手动取消）——**不等 runner**：Windows 产物直接本地 `cargo dist build --tag vX.Y.Z` 构建后 `gh release upload`（`target/distrib/` 下连 `source.tar.gz` 与 installer 脚本都会生成），桌面安装包（`just desktop-build` 产物在 `desktop/src-tauri/target/release/bundle/{msi,nsis}/`）同法上传；tauri 生成的文件名带空格（`Obsidian Export_…`），按 v26.8.3 起惯例改名点连接（`Obsidian.Export_…`）再传；macOS/Linux 产物由 dispatch 的 workflow 排队慢慢补齐即可。**（26.9.1 实况）**runner 紧张时连 Windows 的 build job 也一起排队（四平台全部 queued 超 6 分钟）——不等依然成立：`gh release create vX.Y.Z --notes-file` 直接建 published release + 本地 upload Windows 产物，workflow 排到后 announce 补齐多平台。另注意 `git push origin desktop --tags` 会因试图推全部本地 tag 而报 access rights 失败，分支与 tag 分开推。`target/distrib/` 里可能残留**旧版本**的桌面安装包（cargo dist 只清自己的产物），上传前核对文件名版本号。**（26.9.2 实况）**runner 紧张是持续现状：v26.9.1 的 tag 触发 run 排队 24h+ 从未跑成、其 macOS/Linux 产物至今缺失，v26.9.2 同样仅靠本地 Windows 产物先行。本地双路并行构建安全（根 `target/` 与 `desktop/src-tauri/target/` 是独立目录、互不抢锁）：`cargo dist build` 与 `just desktop-release` 同时后台跑。本地直合的变更要拿 PR 号供 CHANGELOG 引用时：先推 feature 分支并 `gh pr create`，再推 desktop——GitHub 检测 head commits 已可达 base 会自动标记 merged，PR #34 即此法所得。
  - `make-new-release` 在 Git Bash 下会因 `just_executable()` 返回的反斜杠路径被 bash 吞掉而失败；改为手动分步执行其等价步骤：`just set-version` → `uvx towncrier==24.8.0 build --version X.Y.Z --yes` → `bash docs/generate.sh` → 提交 → `git tag vX.Y.Z`。CHANGELOG 片段必须是新式命名 `<issue>.<type>.md`（旧式 `<type>.<issue>.md` 不被识别且**静默跳过**，v26.8.4 曾手工补并入漏掉的 check-json 条目）。
  - **README 是双语的**：`bash docs/generate.sh` 用 obsidian-export 自身导出 docs vault，展开 `docs/_combined.md` 与 `docs/_combined.zh.md` 生成根 `README.md`（英文）与 `README.zh.md`（中文），两份顶部互为语言入口。改 README 一律改 `docs/` 下对应源（中文为 `.zh.md` 成对文件）再重新生成，禁止直接编辑产物；两语言版本内容需人工保持对照。
  - release notes 末尾附「Downloads」资产说明段（GUI 与 CLI 是独立产物、按需下载、其余文件用途），模板见 v26.8.2；桌面与 CLI 的关系（GUI 内置边车、装 GUI 无需另装 CLI）必须写明。写入含反斜杠的路径（如 `%USERPROFILE%\.cargo\bin`）时用文件（`--notes-file`）而非 heredoc，v26.8.2 的 `\b` 曾被 shell 吃掉。
  - 注意 gh CLI 在本仓库目录下无默认 repo 时会解析到 upstream：查 fork 的 release/run 一律带 `-R ONEGAYI/obsidian-export-desktop`。
  - **（26.8.6 实践）CI 与发布的新坑**：
    - fork 的 CI 在 PR #6 前从未全绿（历史 PR 手动合并不等待），Linux 侧存量债一次性清偿后才成为基线：`cfg(not(windows))` 测试本地不可见（平台断言、`tail_expr_drop_order`）、nightly rustfmt 行为演进（imports 粒度变化需全量重排）、tarpaulin 插桩拖慢执行暴露速率类时序断言——本地 Windows 全绿不代表 CI 绿，动测试时留意平台耦合与耗时假设。
    - `docs/CHANGELOG.md` 与 `docs/CONTRIBUTING.md` 是 **git symlink（mode 120000）**：Windows checkout 把它们物化成「内容为目标路径的普通文本文件」，当普通文件编辑（哪怕只追加换行）会污染 blob，Linux 上 symlink 目标带上 `\n` 变 broken（pre-commit 的 check-symlinks 挂）。修复方式 `git hash-object -w --stdin` + `update-index --cacheinfo 120000,<hash>,<path>`；改文件前先 `git ls-files -s` 看 mode。
    - towncrier 片段正文首行**不要带 `- ` 列表前缀**（towncrier 生成时自己加，双前缀 `- - ` 需手工修 CHANGELOG）；生成后条目链接按 issue_format 指向 zoni issues，需手工替换为 fork 的 pull 链接。
    - **（PR #9 实践）rustfmt 工具链已钉 dated nightly `nightly-2026-08-20`**：裸 `nightly` 会随 rustfmt 演进漂移（CI 装到的比本地新就全仓重排、CI 红）。三处同步维护：`Justfile` 的 `rustfmt_toolchain` 变量（`just fmt` 一键安装+格式化）、`.github/workflows/ci.yml` 的 fmt matrix 项、`.pre-commit-config.yaml` 的 rustfmt hook。升级流程：改三处日期 → `just fmt` 全量重排 → 同一提交入库。**本机实况（2026-09）**：`rustup` 下载该 dated 工具链反复失败（链路慢 + 缓存并发损坏 + os error 1450），未能常驻安装；本地日常用 `cargo +nightly fmt` 兜底——当前裸 `nightly`（8925ea358a）与 dated（f7d782a3b）两构建的 rustfmt 行为恰好一致（PR #10 重排经 CI 检查通过为实证），但 `rustup update` 后裸 nightly 前进、与 CI 钉版出现 diff 时，须重新设法安装 dated（或换网络/重试 `just fmt`）而非迁就裸 nightly。另注：一旦 dated 安装成功，先用 `cargo +nightly-2026-08-20 fmt --all -- --check` 核对全仓——若与裸 nightly 的重排有出入（如 comments.rs 的注释宽度差异），以 dated（=CI）为准重新格式化提交。
- 通用行为准则、提交与发布规范以用户级 AGENTS.md 为准，此处不重复。

## 待定事项

- [ ] 嵌入解析缓存与 walker 并行化：vault 索引已消除引用解析的主要瓶颈（基准 7200 文件 11.2s → 0.65s），剩余耗时以文件 IO/解析/渲染为主；两项优化待有真实大 vault 的 profile 数据支撑后再决定是否实施。
- [ ] 预扫描对全 vault 做二次 read+parse（启用渲染时 IO 与解析翻倍，7200 文件基准约 +0.5s；非 Keep 模式经 #28 后还叠加事件物化与 comments 改写）：**不要**走行扫描识别 fence 的方向（免疫规则是结构性的、行扫描必误判，漏计会让原子失败退化成逐块警告——#28 的调研结论）；若要优化，方向是复用 walk/导出阶段的解析结果或文本缓存共享，待 profile 数据支撑。

已知限制与设计取舍（审查登记后决定维持现状的五项：注释转换差异、linkcheck 原文语义、进程树击杀平台不对称、图表副本自包含、EmbedFull 不对称，另含桌面端 ureq watch 项与 Excalidraw 降级链接死链报告项）已迁至 [docs/known-limitations.zh.md](docs/known-limitations.zh.md) 逐条细说（现象/根因/为何不修/绕过方法/源码定位）。26.9.0 发布后的开发期已关闭七项登记（随 26.9.1 发布）：预扫描 comments 感知 #28、`.render-*` 惰性清扫与同文件嵌入全文回退 #27、测试 release 门 const 断言与引用正则兜底 #26、有序列表接续编号 #30、cmd 脚本 `%` 路径自动警告 #31。

## 修复路线（已批准）

按「正确性 → 稳健性 → 性能」三阶段推进（M0 环境基线 → M1 正确性 → M2 稳健性 → M3 性能），彻底完善前不动桌面端业务。GUI 极简原则：缺陷修在 core/CLI 层，桌面端只做展示。三阶段已完成并经两轮子代理审查-修复-复审闭环。

## 桌面端开发（Tauri）

- 常用命令：`just desktop-sync-sidecar`（构建 CLI 并复制到 `desktop/src-tauri/binaries/`，**改动 CLI 后必须重跑**）、`just desktop-dev`、`just desktop-build`、`just desktop-test`、`just desktop-release <tag>`（构建 + 版本校验 + 空格改点 + 上传安装包到 release，`--dry-run` 预览须经 pnpm 直调：`pnpm -C desktop run release -- <tag> --dry-run`）、`just clean <target|desktop|sidecar|all>`（清理中间产物与依赖，范围可选）。
- 前端测试：vitest（`pnpm -C desktop test`，复用 vite.config.ts 的 `@` alias 零额外配置），测试文件与源码同目录（`src/**/*.test.ts`，显式 import vitest API）；含 zh/en 字典占位符集合一致性测试（Widen 宽化抹掉字面量类型，编译期校验不可行，由测试锁定）。
- CI 覆盖：`ci.yml` 的 desktop job（windows-latest）跑 `install → sync-sidecar`（硬顺序：桌面 build script 编译期校验 externalBin，binaries/ 又被 gitignore）`→ tsc+vite build → vitest → cargo test`（桌面 workspace）；rust-cache `workspaces` 同时缓存根 `target/` 与 `desktop/src-tauri/target`（注意 `->` 右侧是 target 目录名非 crate 名，写错会静默失效）。
- 完整构建说明（含 Windows 坑与图标替换）见 [docs/BUILD.md](docs/BUILD.md)（中文，面向人类读者）。
- `desktop/src-tauri` 是独立 cargo workspace（自持 `[workspace]`），不影响根 crate 与 cargo-dist；`.cargo/config.toml` 启用 MSRV 感知解析（工具链锁 1.87）。
- 前端：Tailwind v4 + 手搭 shadcn 层（shadcn CLI 与当前 Node 生态冲突，组件手写在 `src/components/ui/`；CLI 修复后可迁移）。主题三态（light/dark/system，`src/lib/theme.ts`）；用户偏好存 localStorage：路径记忆、「保留根文件夹」（`obsidian-export-*` 逐项键）与转换选项（`obsidian-export-options` 单键 JSON，见 `src/lib/options.ts`）。
- i18n：界面文案抽离为字典（`src/i18n/`，zh 为结构基准、`Widen` 宽化出 `Dict` 类型锁两份字典键一致），运行时经项目首个 React Context（`I18nProvider`）分发；语言三态 zh/en/system（跟随系统按 `navigator.languages` 是否含 zh 前缀判定），偏好存 `obsidian-export-language`，生效语言同步 `document.documentElement.lang`；标题栏下拉（`LanguageMenu`，radix dropdown-menu）三态互转。Rust/CLI 侧英文技术错误原文透传，不进字典。
- 版本号统一由 `just set-version X.Y.Z` 控制：一次对齐六处——根 crate（`Cargo.toml` + `Cargo.lock`）与桌面端三处（`desktop/package.json`、`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/Cargo.toml` + 其 `Cargo.lock`），避免安装包文件名与 release 版本错位（26.8.2 起对齐）。`make-new-release` 已接入该目标。依赖 cargo-edit（仓库工具链锁 1.87 而 cargo-edit 0.13.13 要求 1.92，**须在仓库外目录用 stable 工具链安装 0.13.10**：`rustup run stable cargo install cargo-edit --version 0.13.10 --locked`）；`cargo set-version` 拒绝降级（发布防呆，误 bump 的还原属手动操作）。桌面端 lock 由脚本 sed 直接修补——桌面 workspace 的 build script 依赖已同步的 sidecar 二进制，`cargo check` 在 clean 后不可用。
- 事件流消费遵守 `docs/sidecar-events.md` 契约（导出 / check / update 三种事件方言）；schema 版本常量在 `desktop/src-tauri/src/events.rs` 与 CLI 的 `main.rs` 各有一份，升级时同步改。
- 设置视图为分页式（`OptionsView`：左侧导航六页「转换行为 / 内容过滤 / 文件与过程 / 图表渲染 / 链接检查 / 关于与更新」，窄窗降级横排页签；页签实现 ARIA tabs 模式的 roving tabindex 与方向键导航）。链接检查：导出成功且开关开启时前端自动 invoke `start_check`，检查 vault 源时由 Rust 侧 `build_check_args` 转发与导出一致的非默认过滤项（walk 集对齐；tag 后处理不参与 check），检查导出产物时恒传 `--no-git` 并使 `--hidden` 与导出一致（CLI 默认值本身是过滤——产物目录在 git 仓库内会被 gitignore 静默排除成假阴性）；check 与导出共用 child 槽，`cancel_export` 通杀，`start_export` 返回实际落点供「检查产物」定位；check 流的解析/IO 错误走独立 `check-error` 通道（导出日志视图在检查期已卸载，混入 sidecar-error 会不可见）。

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
│   └── .gitignore # 片段目录占位忽略文件
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
│   │   ├── release-desktop.mjs # 桌面安装包收集改名上传脚本
│   │   └── sync-sidecar.mjs    # 构建 CLI 并同步为 Tauri 边车
│   ├── src/                # 桌面端前端源码（React/TS）
│   │   ├── app-fold.test.ts # 导出事件折叠纯函数测试
│   │   ├── App.tsx          # 应用根组件，串联三阶段导出流程
│   │   ├── components/      # 视图组件目录
│   │   │   ├── ExportDialog.tsx     # 导出前确认对话框
│   │   │   ├── ExportResultView.tsx # 导出结果汇总卡片
│   │   │   ├── ExportRunView.tsx    # 导出进行中的进度与日志视图
│   │   │   ├── link-check.test.ts   # 链接检查状态机纯函数测试
│   │   │   ├── LinkCheckPanel.tsx   # 链接检查面板与状态折叠逻辑
│   │   │   ├── options-view.test.ts # 页签键盘导航纯函数测试
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
│   │   │   ├── update-panel.test.ts # 更新状态机与启动节流测试
│   │   │   └── UpdatePanel.tsx      # 关于与更新页（状态外置）
│   │   ├── i18n/            # 界面国际化目录
│   │   │   ├── en.ts        # 英文字典，键型受 Dict 约束
│   │   │   ├── i18n.test.ts # fmt 与字典占位符一致性测试
│   │   │   ├── index.tsx    # i18n 上下文与语言偏好分发
│   │   │   └── zh.ts        # 中文字典并定义 Dict 类型基准
│   │   ├── index.css        # Obsidian 风格主题变量与全局样式
│   │   ├── lib/             # 前端工具与封装层目录
│   │   │   ├── options.ts # 导出选项类型、校验与摘要
│   │   │   ├── sidecar.ts # Tauri 命令调用与事件封装层
│   │   │   ├── theme.ts   # 主题偏好 Hook，支持跟随系统
│   │   │   └── utils.ts   # cn 类名合并工具函数
│   │   ├── main.tsx         # React 入口（含 i18n）
│   │   └── vite-env.d.ts    # Vite 客户端类型引用
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
│   ├── .obsidian/              # Obsidian 编辑器工作区配置
│   ├── _combined.md            # README 生成的章节嵌入清单
│   ├── _combined.zh.md         # 中文 README 章节嵌入清单
│   ├── _edit-warning.md        # 勿直接编辑 README 的警告块
│   ├── _edit-warning.zh.md     # 中文版勿直接编辑警告块
│   ├── BUILD.md                # 中文构建指南（CLI 与桌面端）
│   ├── CHANGELOG.md            # 指向根变更日志的指针文件
│   ├── changes.md              # 更新日志引导页
│   ├── changes.zh.md           # 中文更新日志引导页
│   ├── contribute.md           # 贡献引导页
│   ├── contribute.zh.md        # 中文贡献引导页
│   ├── CONTRIBUTING.md         # 指向根贡献指南的指针文件
│   ├── desktop.md              # Tauri 桌面端功能介绍
│   ├── desktop.zh.md           # 中文桌面端功能介绍
│   ├── generate.sh             # 双语 README 生成脚本
│   ├── installation.md         # 安装与升级指南
│   ├── installation.zh.md      # 中文安装与升级指南
│   ├── intro.md                # 项目简介与核心特性列表
│   ├── intro.zh.md             # 中文简介与核心特性列表
│   ├── known-limitations.zh.md # 已知限制与设计取舍细说（中文单份）
│   ├── license.md              # 许可证说明
│   ├── license.zh.md           # 中文许可证说明
│   ├── Release-checklist.md    # 发布流程检查清单
│   ├── sidecar-events.md       # 边车 JSON 事件流契约文档
│   ├── usage-advanced.md       # CLI 高级选项与技巧
│   ├── usage-advanced.zh.md    # 中文高级选项与技巧
│   ├── usage-basic.md          # CLI 基本用法说明
│   ├── usage-basic.zh.md       # 中文基本用法说明
│   ├── usage-library.md        # Rust 库使用指引
│   └── usage-library.zh.md     # 中文库使用指引
├── Justfile                # 核心任务入口（桌面端+发布）
├── LICENSE                 # 上游许可证全文
├── README.md               # 英文自述（generate.sh 产物）
├── README.zh.md            # 中文自述（generate.sh 产物）
├── rust-toolchain.toml     # 固定 Rust 工具链版本
├── rustfmt.toml            # 代码格式化规则（需 nightly）
├── src/                    # Rust 源码（CLI 与库）
│   ├── comments.rs       # Obsidian 注释识别与改写状态机
│   ├── context.rs        # 笔记解析上下文与嵌套追踪
│   ├── diagrams.rs       # 图表渲染：渲染器注册表与外部工具编排
│   ├── excalidraw.rs     # Excalidraw 检测解压与转换
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
│   ├── excalidraw_cli_test.rs # Excalidraw CLI 集成测试
│   ├── export_test.rs         # 库级导出功能集成测试
│   ├── postprocessors_test.rs # 后处理器行为测试
│   └── testdata/              # 测试 vault fixtures
│       ├── expected/ # 黄金输出树（10 个场景）
│       │   ├── comments/                            # 注释转换黄金输出树
│       │   ├── comments-strip/                      # 注释移除模式黄金输出树
│       │   ├── filter-by-tags/                      # 标签过滤黄金输出树
│       │   ├── infinite-recursion/                  # 循环嵌入降级输出树
│       │   ├── main-samples/                        # 主样例黄金输出树
│       │   ├── non-ascii/                           # 非 ASCII 黄金输出树
│       │   ├── postprocessors/                      # 后处理器黄金输出树
│       │   ├── same-filename-different-directories/ # 同名消解黄金输出树
│       │   ├── single-file/                         # 单文件黄金输出树
│       │   └── start-at/                            # start-at 黄金输出树
│       └── input/…   # 测试输入 vault（31 场景子目录）
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
