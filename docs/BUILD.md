# 构建指南

本文介绍如何从源码构建本项目，包括 CLI、Rust 库与桌面端（Tauri GUI）。

## 环境要求

| 工具 | 版本 | 说明 |
|------|------|------|
| Rust | 1.87（以 `rust-toolchain.toml` 固定为准） | rustup 会自动安装指定版本 |
| pnpm | ≥ 9 | 桌面端前端依赖管理与命令入口 |
| just | 1.53.x | 常用任务入口（见下文 Windows 注意事项） |
| uvx | 可选 | 仅发布 CHANGELOG 时需要（towncrier） |

桌面端首次构建前，进入 `desktop/` 安装前端依赖：

```bash
pnpm -C desktop install
```

## CLI 与库

根目录是一个标准 Rust crate（CLI 与库同仓）：

```bash
cargo build          # 构建
cargo test           # 全量测试（含集成测试）
cargo clippy --all-targets  # lint（CI 零警告标准）
```

依赖版本受 MSRV 约束：`desktop/src-tauri/.cargo/config.toml` 中的
`resolver.incompatible-rust-versions = "fallback"` 只影响桌面端 workspace，
根 crate 的构建不受影响。

## 桌面端（Tauri）

桌面端采用「桌面端 + 边车」架构：GUI 不实现任何转换逻辑，启动时把
`obsidian-export` CLI 作为子进程（sidecar）调用，消费其
`--progress json` 事件流（契约见 [sidecar-events.md](sidecar-events.md)）。

### 日常开发

```bash
just desktop-dev     # 同步边车二进制并以 dev 模式启动应用
```

`desktop-dev` 会先执行 `desktop-sync-sidecar`：以 release 构建 CLI、按
当前平台 triple 复制为 `desktop/src-tauri/binaries/obsidian-export-<triple>.exe`。
**改动 CLI 代码后需重启 dev 会话才会生效**（同步只发生在启动时）。

### 测试与打包

```bash
just desktop-test    # 运行桌面端 Rust 单元测试（events 解析、路径解析等）
just desktop-build   # 同步边车并打包正式版安装包（输出见 desktop/src-tauri/target）
```

### 清理

`just clean <范围>` 快捷清理中间产物与依赖，范围可选：

| 范围 | 清理内容 |
|------|----------|
| `target` | 根目录 `target/`（CLI 与库的构建缓存） |
| `desktop` | `desktop/src-tauri/target`、`node_modules`、`dist`、`gen` |
| `sidecar` | `desktop/src-tauri/binaries`（重跑 `desktop-sync-sidecar` 即恢复） |
| `all` | 以上全部 |

清理 `sidecar` 或 `desktop` 后，下次 `just desktop-dev` 会自动重建所需产物。

`desktop/src-tauri` 是独立的 cargo workspace（自持 `[workspace]`），与根
crate 的构建、cargo-dist 发布流水线互不影响。

### 替换应用图标

准备一张 1024×1024 透明底 PNG，运行：

```bash
pnpm -C desktop tauri icon <图标路径.png>
```

该命令自动生成全套尺寸并覆盖 `desktop/src-tauri/icons/`，下次构建生效。

各产物的图标表现（26.8.2 后的配置）：

- 主程序 exe 与安装后的快捷方式：取 `icons/icon.ico`，正常生效。
- NSIS 安装包（`*-setup.exe`）：由 `tauri.conf.json` 的
  `bundle.windows.nsis.installerIcon` 显式指定为 `icons/icon.ico`——Tauri 2
  该项**无默认值**，不配置时安装包文件显示 NSIS 通用图标。
- MSI 安装包：资源管理器中 `.msi` 文件一律显示**系统通用 MSI 图标**，
  这是 Windows 平台行为（文件级图标不随包内嵌），无法通过 Tauri 配置
  改变；安装后的程序列表与快捷方式图标不受影响，仍为应用图标。

## Windows 注意事项

- **just 版本**：1.53–1.57 在 Windows 存在 shebang 临时文件路径 bug（丢失反斜杠，
  报 exit 127），而 1.58+ 需要 rustc ≥ 1.89。推荐固定 1.53.x，且 Justfile 中的
  桌面端任务已改为单行 recipe 并显式指定 `windows-shell` 为 powershell.exe。
- **PATH 丢失**：若经由 bash 链式调用导致 cargo/rustc 找不到，直接在
  PowerShell 中运行 `just desktop-dev`；同步脚本本身是纯 Node 实现，不经 shell。
- **端口占用**：dev 模式依赖 1420 端口。上一次会话异常退出后若报
  `Port 1420 is already in use`，用 `netstat -ano | findstr :1420` 找到 PID 后
  `taskkill /PID <pid> /F`，再重新启动。

## 发布流程（维护者）

1. `changelog.d/` 下按 towncrier 片段类型追加变更说明；
2. 更新 `Cargo.toml` 版本（日历版本 `yy.m.i`）并 `cargo check` 刷新 lock；
3. `uvx towncrier==24.8.0 build --version <版本> --yes` 生成 CHANGELOG；
4. `bash docs/generate.sh` 重新生成 README；
5. 提交并打 tag（参见 Justfile 中 `make-new-release` 的完整流程）。

### 上传桌面安装包

tag 推送、release 已建（或至少 tag 已推送）后，一条命令完成桌面安装包的
构建、改名与上传：

```powershell
just desktop-release vX.Y.Z
```

等价于 `just desktop-build` + 收集 `bundle/{msi,nsis}/` 产物 + 文件名空格
改点连接 + `gh release upload vX.Y.Z … --clobber -R ONEGAYI/obsidian-export-desktop`。
脚本会先校验每个产物文件名含目标版本号（防止版本错位上传），再改名上传；
只想核对清单不上传时加 `--dry-run`：

```powershell
pnpm -C desktop run release -- vX.Y.Z --dry-run
```

其余发布细节（workflow 手动 dispatch、CLI 产物本地构建等）见
[Release-checklist.md](Release-checklist.md)。
