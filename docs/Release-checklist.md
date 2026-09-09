# Release process

Fork 现状要点：**v26.9.0 起 tag 推送已能自动触发 release workflow**（v26.8.x
时代从不触发系当时 tag 模式 `**[0-9]+.[0-9]+.[0-9]+*` 不匹配 `vX.Y.Z`，模式
手改为 `v*.*.*` 并补 `workflow_dispatch` 后已恢复；因此 `dist-workspace.toml`
配了 `allow-dirty = ["ci"]` 放行 cargo-dist 对 release.yml 的漂移检查，0.28
语法为列表、布尔值会 TOML 报错。勿在 tag 已触发时再手动 dispatch，会产生
重复 run 需取消其一）；**不等 runner 排队**（macOS/Linux 曾排队数小时），
Windows 产物一律本地构建上传。

## 版本与变更登记

- [ ] `changelog.d/` 追加 towncrier 片段（新式命名 `<issue>.<type>.md`；旧式
      `<type>.<issue>.md` **不被识别且静默跳过**，v26.8.4 曾因此漏掉 check-json
      条目；正文首行不要带 `- ` 列表前缀，towncrier 生成时会自己加）
      片段生命周期：PR 合并时登记、下次发布时被 towncrier 消费删除——目录里
      存在上次发布之后登记的片段是**正常待发状态**，不是残留；发布完成后目录
      应只剩 `.gitignore`
- [ ] `just set-version X.Y.Z` 一次对齐六处版本（根 crate 两处 + 桌面端四处）；
      CalVer `YY.MM.PATCH` 不得跳月，`just set-version` 拒绝降级。依赖
      cargo-edit（仓库工具链锁 1.87 而 0.13.13 要求 1.92，**须在仓库外目录用
      stable 工具链安装 0.13.10**：
      `rustup run stable cargo install cargo-edit --version 0.13.10 --locked`）
- [ ] `uvx towncrier==24.8.0 build --version X.Y.Z --yes` 生成 CHANGELOG；
      条目里的 issue 链接按 issue_format 指向上游，需手工替换为 fork 的 pull
      链接；版本标题与第一个分类之间补 1-2 句版本总结；底部 `<!-- 变更链接 -->`
      处追加 compare 链接
- [ ] `bash docs/generate.sh` 重新生成双语 README（改 README 只改 `docs/` 源）
- [ ] 提交并 `git tag vX.Y.Z`（`just make-new-release` 在 Git Bash 下会因
      `just_executable()` 返回的反斜杠路径被 bash 吞掉而失败，手动分步执行其
      等价步骤：set-version → towncrier → generate.sh → 提交 → tag）

## 发布与产物

- [ ] push commit 与 tag 到 origin（fork）——**分支与 tag 分开推**（`git push
      origin desktop` + `git push origin vX.Y.Z`；`--tags` 会试图推全部本地
      tag 而 access rights 失败），然后
      `gh run list --workflow=release.yml -R ONEGAYI/obsidian-export-desktop`
      观察：tag 已自动触发即无需 dispatch（tag 触发走 tag 所指 commit 上的
      workflow 定义，tag 必须指向含最新 workflow 的提交；macOS/Linux CLI 产物
      由它排队慢慢补齐）。确需手动时：
      `gh workflow run release.yml --ref vX.Y.Z -R ONEGAYI/obsidian-export-desktop`
- [ ] runner 紧张是持续现状（26.9.1 的 tag run 排队 24h+ 从未跑成、四平台
      job 全部 queued 也出现过）——**不等**：`gh release create vX.Y.Z
      --notes-file <file>` 直接建 published release，Windows 产物本地构建后
      upload，macOS/Linux 由 workflow 排到后补齐
- [ ] CLI Windows 产物：本地 `cargo dist build --tag vX.Y.Z`，然后
      `gh release upload vX.Y.Z target/distrib/<文件…> -R ONEGAYI/obsidian-export-desktop`
      （`target/distrib/` 下连 `source.tar.gz` 与 installer 脚本都会生成）。
      **上传前核对文件名版本号**：该目录可能残留旧版本的桌面安装包（cargo
      dist 只清自己的产物）
- [ ] 桌面安装包：`just desktop-release vX.Y.Z`（构建 + 空格改点 + 版本校验 +
      上传；先 `pnpm -C desktop run release -- vX.Y.Z --dry-run` 核对清单）。
      与 `cargo dist build` **双路并行安全**——根 `target/` 与
      `desktop/src-tauri/target/` 是独立目录互不抢锁，后台同时跑
- [ ] release notes：`gh release create/edit vX.Y.Z --notes-file <file> -R
      ONEGAYI/obsidian-export-desktop`。notes 必须含该版本完整 CHANGELOG 与
      「Downloads」资产说明段（GUI 与 CLI 是独立产物、装 GUI 无需另装 CLI，
      模板见 v26.8.2）；含反斜杠的路径（如 `%USERPROFILE%\.cargo\bin`）用
      `--notes-file` 写入，不要 heredoc（`\b` 等会被 shell 吃掉）
- [ ] 核对 release 资产齐全：CLI 四平台压缩包 + installer 脚本 +
      `source.tar.gz` + 桌面 msi/nsis

> 注意：在本仓库目录下 gh 无默认 repo 时会解析到 upstream，查/传 fork 的
> release 一律带 `-R ONEGAYI/obsidian-export-desktop`。
