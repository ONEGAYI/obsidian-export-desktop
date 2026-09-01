# 安装

## 预编译二进制

CLI 的预编译产物，以及 Windows 图形界面桌面端安装包，均可在 <https://github.com/ONEGAYI/obsidian-export-desktop/releases> 下载。

桌面端已内置 CLI 作为边车进程：安装桌面端即可直接使用，只有在想从终端调用 CLI 时才需要单独安装。

## 从源码构建

当你的平台没有预编译产物，或者你不信任预编译二进制时，_obsidian-export_ 也可以轻松地从源码编译。
通过 Rust 官方包管理器 [Cargo] 完成，步骤如下：

1. 从 <https://www.rust-lang.org/tools/install> 安装 Rust 工具链
2. 克隆本仓库
3. 在仓库根目录运行 `cargo install --path .`

> 安装 Rust 工具链时应按 <https://www.rust-lang.org/tools/install> 上「Configuring the PATH environment variable」一节的说明正确配置 PATH 变量。

## 从旧版本升级

下载了预编译二进制的用户，下载最新版本替换旧文件即可——也可以让 CLI 代劳：`obsidian-export update --download`。

从源码构建的用户，拉取最新代码后再次运行 `cargo install --path .` 即可。

[Cargo]: https://doc.rust-lang.org/cargo/
