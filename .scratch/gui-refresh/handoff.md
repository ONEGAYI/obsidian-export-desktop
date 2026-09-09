# GUI 美化：实现交接

## 目标与当前阶段

用户已确认纸白淡紫的界面方向和默认高度单行胶囊，要求按工程流程准备规格，并明确指定 **implement 由另一个 Agent 完成**。本次仅做工作流登记、规格、tickets 与交接。

**远端规格是事实源**：[总规格 #37](https://github.com/ONEGAYI/obsidian-export-desktop/issues/37)。先读取最新 Issue，再看本地参考图；本地 spec 和 tickets 是发布快照，不用于覆盖远端新决定。

## 已完成

- 按用户后续授权启用 fork 的 GitHub Issues。
- 创建总规格 #37；三个原生子任务依次为 [#38 路径预览](https://github.com/ONEGAYI/obsidian-export-desktop/issues/38)、[#39 响应式首页](https://github.com/ONEGAYI/obsidian-export-desktop/issues/39)、[#40 其余页面适配与回归](https://github.com/ONEGAYI/obsidian-export-desktop/issues/40)。
- 原生依赖为 #39 blocked by #38，#40 blocked by #39；#38 为当前可接手票。父规格和 #38 标记 `ready-for-agent`，不代表产品完成。
- 新增 `docs/agents/issue-tracker.md`、`docs/agents/domain.md`，并在 `AGENTS.md` 增加入口；保持 `CLAUDE.md` 通过 `@AGENTS.md` 导入。
- 本地保存最终参考图 `reference.png`，图中示例数据与像素比例不具约束力。以 #37 的文字契约为准。
- 未修改应用实现、未提交、未推送、未创建 PR、未发布版本。

## 接手前先核对

阅读基线为 `desktop` 分支的 `f447ccacdaf0dc1e9b35c0635c9ed9328f98d72d`。先检查实际 HEAD 与工作区；当前文档变更尚未提交，不得把它们当成无关脏文件清除。新建 worktree 不会自动带走这些未提交材料，应先按用户验收/提交约定处理或明确复制所需文件。

最终图位于本文件同目录；SHA-256：`bbc7ed83de4cd16988a4798776fb0ac3d35588da69916a336d2fdda9214e568f`。图尚未托管到远端，换机器/工作区时至少携带 `reference.png` 和本交接文件。不要使用先前消息中 `/C:/...` 的无效路径；Windows 本地读取用 `C:/...` 或 `D:/...`。

## 已锁定的关键约束

- 默认 760×520、最小 560×480；宽度 <1000px 完全隐藏配置详情，>=1000px 才出现右栏。
- 高度 <640px 使用根选项 + 输出路径的一行胶囊，>=640px 展开；宽高独立。640px 是实现初值，改动需证据和规格同步。
- 删除流程介绍；完整配置仍从设置页访问。浅/深/system 主题与中英/system 语言保留。
- 默认真实选项不等于参考图示例：图表和链接检查默认关闭。
- 当前 `ExportDialog` 拼接规则与 Rust 的目录/单文件判断不同。按 #38 新增无副作用只读预览，共用 `resolve_destination` 及启动的绝对路径解析；实际运行后仍以 `start_export` 返回值为准。
- 来源浏览当前只能选目录，单笔记由手动路径输入支持；此次不新增浏览模式。
- 桌面端不做 Markdown 转换；不改 CLI、事件协议、进程槽与取消/更新业务。

## Suggested Skills

1. **implement**：当前首要技能，读取 #37 和 #38 后进入实现。Windows 本机技能路径：`C:/Users/64487/.codex/plugins/cache/openai-curated-remote/matt-skills-curated/1.1.0/skills/implement/SKILL.md`。
2. **tdd**：在实现阶段针对路径与用户行为先写有效的失败测试，避免 className 镜像测试。
3. **file-tree**：新增/移动文件时读仓库 `.agents/skills/file-tree/SKILL.md`，只通过维护脚本更新树。
4. **code-review**：实现结束后按仓库规范与总规格审查；最终由用户验收。

无需重复使用 to-spec、重新生成参考图或再次访谈。若真实代码与基线不同，先解释具体差异，不擅自扩展产品范围。

## 下一步命令与接手提示

在仓库根目录先执行只读命令：

```powershell
git status --short
git rev-parse HEAD
gh issue view 37 -R ONEGAYI/obsidian-export-desktop --comments
gh issue view 38 -R ONEGAYI/obsidian-export-desktop --comments
```

可直接交给下一个 Agent 的提示：

> 请读取 `.scratch/gui-refresh/handoff.md`、根 `AGENTS.md` 和 GitHub 总规格 #37。使用 implement，从 #38 开始，依次完成 #38 → #39 → #40；查看本地 `reference.png`。保护当前未提交文档和其他人的改动，按现有约定组织一个 PR。完成自动化测试、真实视口验收与 code-review 后交给用户验收，不擅自发布版本。以最新远端 Issues 为规格与任务事实源。

前置 ticket 的代码和验收可在同一未合并分支完成。记录提交与验证证据后再解除依赖、更新就绪标签；不需要为这条顺序任务链创建多个 PR。总规格保持打开直到实现与用户验收完成。

## 验证与未执行事项

本轮仅验证规划材料、链接、文件树和 GitHub 父子/依赖结构；最终验证结果见本节追加记录。应用 build、Vitest、Rust 测试和真实窗口适配均由实现 Agent 按 #37 执行，当前没有任何“实现已通过”的结论。

文件树首次检查发现既有 `changelog.d/35.new.md`、`36.new.md` 未收录，本轮按现有内容补登记，不修改这两个片段正文。

2026-09-09 验证记录：

- `tree_tool.py check --strict` 通过，规范形态、词表、关联、磁盘与渲染产物一致。
- `git diff --check` 通过；8 份 Markdown 的相对链接和围栏检查无错误。
- GitHub 只读回查确认 Issues 已启用；#37 的原生子任务为 #38/#39/#40；#39、#40 的原生阻塞分别为 #38、#39。
- 四个 Issue 都保持 open；#38 是唯一没有前置依赖的实现 ticket。
- 应用源码与测试没有改动；本轮不运行应用测试套件，避免把规划验证写成实现验收。
