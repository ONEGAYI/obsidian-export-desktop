# Issue 与规格管理

## 仓库与工具

- 主仓库：`ONEGAYI/obsidian-export-desktop`，GitHub；本地 `origin` 指向该 fork。
- 上游：`zoni/obsidian-export`；本地 `upstream` 仅用于上游同步与贡献。
- 自有开发的集成分支：`desktop`。新功能分支默认使用 `codex/` 前缀。
- GitHub 操作优先使用已连接的 GitHub MCP；不覆盖所需能力时使用 `gh`。
- 使用 `gh` 查询或写入 fork 时，显式传入 `-R ONEGAYI/obsidian-export-desktop`，不要依赖自动推断。

## GitHub 为规格与任务的事实源

**规格（spec）与实现任务（ticket）使用 GitHub Issues 管理**。总规格使用 `[Spec]` 标题前缀，任务使用 `[Ticket]` 前缀；使用原生 sub-issue 关联，并登记依赖。原生依赖不可用时，保留正文 `Blocked by:` 和父规格的任务清单，不以不存在的关联冒充配置完成。

当技能要求「发布规格 / tickets」时，在用户授权的工作范围内创建 GitHub Issues。总规格保存产品范围、决定和验收条件；任务保存独立交付物、依赖和二元验收清单。更新决定先改总规格，再同步受影响任务，不维护平行的本地规格事实源。

本地 `.scratch/<feature>/` 仅保留首次发布快照、参考附件和 `handoff.md`。本仓库未忽略 `.scratch/`，文件须由 file-tree 登记；当前未提交材料在换 worktree 或机器前必须显式携带。它们尚未被推送时，不能在远端 Issue 中写成可访问的仓库附件链接。

2026-09-09 用户明确同意用 GitHub Issues 管理 specs 和 tickets，本次已据此启用原本关闭的 Issues。该授权不等于代码实现、推送或发布版本的授权；当前规划工作不创建 PR。

多行 Issue/PR 正文优先使用结构化工具参数；使用 `gh` 时先写正文文件，再传 `--body-file`。提交、PR 语言及人类验收要求遵循根 `AGENTS.md` 和用户级规则。

## 状态与标签

规格正文登记 `draft`、`ready-for-agent`、`in-progress`、`ready-for-human`、`done` 等状态。新增 `ready-for-agent` 标签表示说明已就绪；总规格可有该标签，但只是组织入口，执行顺序以子任务为准。只给无阻塞任务添加该标签。被阻塞任务的前置交付可在同一未合并分支完成，实际解除时需记录提交与验证证据，再更新依赖和标签，不机械等待同一最终 PR 合并。

2026-09-09 只读核查的远端标签为：`accessibility`、`bug`、`documentation`、`duplicate`、`enhancement`、`good first issue`、`help wanted`、`invalid`、`question`、`wontfix`。后续使用前重新核对实际标签，不将这些标签推断为一套已配置的 triage 流程。

仓库内当前只部署了 file-tree 技能，没有部署 triage 技能；因此本次不创建完整 `triage-labels.md`。仅为本次规格/任务流程新增 `ready-for-agent`，保留所有既有标签；若以后部署 triage，再映射现有标签并补齐五态配置。
