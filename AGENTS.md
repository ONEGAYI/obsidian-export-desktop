# AGENTS.md

## 项目定位

本项目基于上游开源项目 [zoni/obsidian-export](https://github.com/zoni/obsidian-export)（Rust 实现的 Obsidian vault → 通用 Markdown 导出工具）改造而来，目标是为它补齐一个**桌面图形界面**。

## 改造目的：桌面端 + 边车架构

整体采用「桌面端 + 边车（sidecar）」双进程架构。

> 边车（sidecar）：随主程序一同运行、承担具体业务的辅助进程。本项目中边车就是 obsidian-export 的 CLI。

| 部分 | 形态 | 职责 |
|------|------|------|
| 桌面端 | 待新增 | 一切 GUI 事务：界面渲染、文件与文件夹选择、转换选项配置、进度与结果展示 |
| 边车 | 已有 CLI | 实际业务：遍历 vault，将 Obsidian 方言语法（wikilink 双链、嵌入等）转换为通用 Markdown。**callout（`> [!NOTE]` 等）不在转换范围内，导出时原样保留**，通用 Markdown 渲染器会将其显示为字面文本 |

职责边界是硬约束：

- 桌面端**不得**实现任何转换逻辑，一律通过启动边车进程、传递参数并读取输出完成。
- 边车**保持可独立使用**：在终端单独运行时功能完整，桌面端只是它的一种调用方式。

## 仓库约定

- 本仓库克隆自上游，`origin` 指向 zoni/obsidian-export。面向上游的 issue/commit/PR 使用英文；本地自有提交与文档使用中文。
- 推送前需先决定远端策略（fork 或新建仓库），届时更新此处说明。
- 通用行为准则、提交与发布规范以用户级 AGENTS.md 为准，此处不重复。

## 待定事项

- [ ] 桌面端技术栈（候选：Tauri / Electron / WPF 等），确定后更新架构描述与文件树。
- [ ] 块引用内容提取增强：`![[note#^block]]` 目前不做真正的块定位，匹配不到块 id 时按 `--missing-section` 策略处理（src/lib.rs 中留有 TODO 指引）。
- [ ] serde_yaml 迁移：当前依赖 0.9.34（上游已归档停维，无安全修复通道），且属公共 API（`pub use serde_yaml`），迁移属破坏性变更，需单独评估社区维护 fork（如 serde_norway）。

## 修复路线（已批准）

按「正确性 → 稳健性 → 性能」三阶段推进（M0 环境基线 → M1 正确性 → M2 稳健性 → M3 性能），彻底完善前不动桌面端业务。GUI 极简原则：缺陷修在 core/CLI 层，桌面端只做展示。

## 文件树

```text
obsidian-export/
├── src/                 # Rust 源码（CLI 与库）
│   ├── main.rs          # CLI 入口（bin）：参数解析与命令行交互
│   ├── lib.rs           # 库入口：组装导出流程
│   ├── context.rs       # 导出上下文与配置
│   ├── frontmatter.rs   # frontmatter 的解析与剥离
│   ├── postprocessors.rs# 后处理器：对导出结果再加工
│   ├── references.rs    # 引用解析：wikilink、嵌入等链接形式
│   └── walker.rs        # vault 的递归遍历
├── tests/               # 集成测试
├── docs/                # 项目文档
├── changelog.d/         # towncrier 的 changelog 片段
├── .github/             # CI 工作流
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
