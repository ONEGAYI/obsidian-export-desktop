# Release process

Fork 现状要点：**v26.9.0 起 tag 推送已能自动触发 release workflow**（v26.8.x
时代从不触发系当时 tag 模式 `**[0-9]+.[0-9]+.[0-9]+*` 不匹配 `vX.Y.Z`，模式
修正为 `v*.*.*` 后已恢复；勿在 tag 已触发时再手动 dispatch，会产生重复 run
需取消其一）；**不等 runner 排队**（macOS/Linux 曾排队数小时），Windows 产物
一律本地构建上传。

## 版本与变更登记

- [ ] `changelog.d/` 追加 towncrier 片段（新式命名 `<issue>.<type>.md`；正文
      首行不要带 `- ` 列表前缀，towncrier 生成时会自己加）
- [ ] `just set-version X.Y.Z` 一次对齐六处版本（根 crate 两处 + 桌面端四处）；
      CalVer `YY.MM.PATCH` 不得跳月，`just set-version` 拒绝降级
- [ ] `uvx towncrier==24.8.0 build --version X.Y.Z --yes` 生成 CHANGELOG；
      条目里的 issue 链接按 issue_format 指向上游，需手工替换为 fork 的 pull 链接
- [ ] `bash docs/generate.sh` 重新生成双语 README（改 README 只改 `docs/` 源）
- [ ] 提交并 `git tag vX.Y.Z`（`just make-new-release` 在 Git Bash 下不可用，
      手动分步执行其等价步骤）

## 发布与产物

- [ ] push commit 与 tag 到 origin（fork），然后
      `gh run list --workflow=release.yml -R ONEGAYI/obsidian-export-desktop`
      观察：tag 已自动触发即无需 dispatch（tag 触发走 tag 所指 commit 上的
      workflow 定义，tag 必须指向含最新 workflow 的提交；macOS/Linux CLI 产物
      由它排队慢慢补齐）。确需手动时：
      `gh workflow run release.yml --ref vX.Y.Z -R ONEGAYI/obsidian-export-desktop`
- [ ] CLI Windows 产物：本地 `cargo dist build --tag vX.Y.Z`，然后
      `gh release upload vX.Y.Z target/distrib/<文件…> -R ONEGAYI/obsidian-export-desktop`
      （`target/distrib/` 下连 `source.tar.gz` 与 installer 脚本都会生成）
- [ ] 桌面安装包：`just desktop-release vX.Y.Z`（构建 + 空格改点 + 版本校验 +
      上传；先 `pnpm -C desktop run release -- vX.Y.Z --dry-run` 核对清单）
- [ ] release notes：`gh release edit vX.Y.Z --notes-file <file> -R
      ONEGAYI/obsidian-export-desktop`。notes 必须含该版本完整 CHANGELOG 与
      「Downloads」资产说明段（GUI 与 CLI 是独立产物、装 GUI 无需另装 CLI）；
      含反斜杠的路径（如 `%USERPROFILE%\.cargo\bin`）用 `--notes-file` 写入，
      不要 heredoc（`\b` 等会被 shell 吃掉）
- [ ] 核对 release 资产齐全：CLI 四平台压缩包 + installer 脚本 +
      `source.tar.gz` + 桌面 msi/nsis

> 注意：在本仓库目录下 gh 无默认 repo 时会解析到 upstream，查/传 fork 的
> release 一律带 `-R ONEGAYI/obsidian-export-desktop`。
