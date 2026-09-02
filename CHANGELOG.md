# Changelog

<!-- towncrier release notes start -->

## [26.9.1](https://github.com/zoni/obsidian-export/tree/26.9.1) - 2026-09-02

Quality release: two correctness fixes for features introduced in 26.9.0 (ordered-list numbering resuming across block comments, comment-aware diagram tool prescan), plus desktop-side robustness fixes (stderr decoding, cancellation states, tab keyboard navigation) and a one-command desktop release helper.

### New Features

- 新增桌面安装包一键发布命令 just desktop-release vX.Y.Z：构建、按版本校验、文件名空格改点、上传到 GitHub release 一条链完成（--dry-run 可预览清单）；发布检查清单重写为 fork 现状（v26.9.0 起 tag 推送已自动触发 release workflow，勿再叠加手动 dispatch）。 ([#21](https://github.com/ONEGAYI/obsidian-export-desktop/pull/21))

### Changes

- 桌面端设置页的页签支持方向键与 Home/End 键盘导航（ARIA tabs 模式的 roving tabindex）：方向键环绕移动并直接切页，Tab 键只停在当前页签上。 ([#20](https://github.com/ONEGAYI/obsidian-export-desktop/pull/20))
- cargo test --release 下图表与 update 集成测试不再静默消失：原先被 debug-only 注入钩子门控的测试文件在 release 下编译为空（0 个测试、静默通过，update 测试更会直连真实 GitHub API 假失败），现改为显式编译失败并提示改跑 cargo test。 ([#26](https://github.com/ONEGAYI/obsidian-export-desktop/pull/26))

### Fixes

- 桌面端边车错误输出的解码与内存修复：stderr 改为字节累积、进程结束时一次性解码，chunk 边界切断中文等多字节字符不再产生乱码替换符；并加 64KiB 上限（截断保留尾部并标注丢弃字节数），卡死进程无限写错误输出不再拖垮内存。 ([#17](https://github.com/ONEGAYI/obsidian-export-desktop/pull/17))
- 桌面端两处时序修复：窗口最小化时链接检查进度不再停滞（动画帧回调被暂停的场合由定时器兜底冲刷）；导出进行中切换界面语言不再产生重复的完成计数与日志行（事件订阅不再随语言重建）。 ([#19](https://github.com/ONEGAYI/obsidian-export-desktop/pull/19))
- 桌面端链接检查与更新下载运行中取消时，结果不再误显示为「未能完成」的失败态，而是明确的「已取消」态；已取消的下载可直接重新下载。 ([#23](https://github.com/ONEGAYI/obsidian-export-desktop/pull/23))
- 图表渲染临时文件残留清扫与同文件嵌入解析修复：进程被杀或崩溃遗留的 .render-* 垃圾会在下次导出同一目录时自动清掉（按 10 分钟年龄阈值惰性清扫，并行安全）；跨文件嵌入片段内的同文件引用（如 ![[note#S]] 片段内的 ![[#Other]]）改按全文件解析，与 Obsidian 语义对齐，不再因目标不在片段内而误报缺失塌缩。 ([#27](https://github.com/ONEGAYI/obsidian-export-desktop/pull/27))
- 图表工具预扫描感知 --comments 模式：位于 %% 注释内的图表代码块在 strip/convert 下不会渲染，因此不再计入总数、也不再要求安装对应工具——此前「唯一图表块在注释内且本机缺工具」会导致整个导出直接失败，即使该块根本不会出现在产物中；keep 模式行为不变，缺工具时真实会渲染的块仍按原契约原子化报错。 ([#28](https://github.com/ONEGAYI/obsidian-export-desktop/pull/28))
- 注释转换打断有序列表后编号接续：convert 模式下块级注释合成的 HTML 块会把产物列表物理分段，此前分段后一段重新从起始编号显示（1、2 而非接续的 2、3）；现在注释重开点显式关闭列表并按已发出的列表项数量合成接续起始编号（嵌套列表只分段被打断的最内层，外层保持连续；注释区间内新开的嵌套列表保持自身起始编号，同一列表被多次打断时编号逐段累计）。strip 模式产物列表本就保持连续、编号按位置递增，行为不变；无序列表不受影响，仅产物在列表与注释块之间多了规范的空行分隔（渲染等价）。 ([#30](https://github.com/ONEGAYI/obsidian-export-desktop/pull/30))
- cmd 脚本工具路径含 `%` 时导出自动警告：Windows 的 cmd.exe 包装会把成对 `%` 当作环境变量展开（引号不保护、`%%` 加倍转义不可靠），含 `%` 的 `.cmd`/`.bat` 工具路径可能被静默改写导致渲染异常；现在工具解析阶段对此类路径发出非致命警告（CLI 输出到 stderr、桌面端走既有 warning 事件通道），建议用 `--diagram-bin` 指向底层 `.exe` 绕过包装。判定按扩展名与路径字符进行、跨平台一致，导出本身不受影响。 ([#31](https://github.com/ONEGAYI/obsidian-export-desktop/pull/31))


## [26.9.0](https://github.com/zoni/obsidian-export/tree/26.9.0) - 2026-09-02

This release adds two features: diagram rendering (dot/mermaid/wavedrom/tikz code blocks rendered to image assets via local tools and embedded in the output) and three-state handling of Obsidian comments (`%%` fenced: keep, convert to HTML comments, or strip). The README also gained a Chinese edition.

### New Features

- 导出时将 Obsidian 特殊代码块（dot/mermaid/wavedrom/tikz）通过本机工具渲染为图片资产并嵌入产物：内容寻址命名与增量缓存、png 模式按渲染器回落 svg、单块渲染失败保留原代码块、工具缺失在预扫描阶段原子化报错（零输出）。工具优先显式路径（`--diagram-bin`），回落 PATH 查找（Windows 含 PATHEXT 与 `.cmd` 包装执行）；导出事件流新增 `diagram-render` 进度事件；桌面端设置新增「图表渲染」页（药丸复选 + 格式与路径覆盖）。([#9](https://github.com/ONEGAYI/obsidian-export-desktop/pull/9))
- 导出时可转换 Obsidian 注释（`%%` 百分号围栏）：新增 `--comments keep|convert|strip` 三态选项（默认 keep 保持现状）。convert 模式将注释改写为 HTML 注释（`<!-- -->`，源码保留、渲染隐藏），strip 模式彻底移除。识别遵循 Obsidian 纯文本配对语义（非贪心、跨空行与列表/引用块边界、未闭合保持字面），代码块、行内代码、公式、表格与链接文本内不识别；注释内容中会破坏 HTML 注释语法的 `--` 序列自动中和。库用户经 `obsidian_export::postprocessors::obsidian_comments(CommentsMode)` 挂载同一后处理器；桌面端「转换行为」设置页新增三态单选。([#12](https://github.com/ONEGAYI/obsidian-export-desktop/pull/12))


## [26.8.7](https://github.com/zoni/obsidian-export/tree/26.8.7) - 2026-08-28

Patch release for the reference parser: wikilinks that escape their alias pipe as `\|` (required inside Markdown tables) now resolve normally instead of being treated as missing.

### Fixes

- 修复 Obsidian 表格内 wikilink 别名转义竖线（``[[note\|alias]]``）的解析：反斜杠不再残留在文件名中导致链接误判失效。此前该写法的链接在导出时会降级为斜体纯文本，并使 ``obsidian-export check`` 误报「目标不存在」；现在导出正常生成相对链接，检查归零。([#7](https://github.com/ONEGAYI/obsidian-export-desktop/pull/7))


## [26.8.6](https://github.com/zoni/obsidian-export/tree/26.8.6) - 2026-08-27

This release adds update checking and downloading for both the CLI and the desktop app: a new `obsidian-export update` subcommand (with a machine-readable `--progress json` event stream) and an "About & update" settings page that checks GitHub releases on launch and can download and run the new installer.

### New Features

- Update check & download, for both the CLI and the desktop app

  `obsidian-export update` checks GitHub for a newer release and can download it (`--download`, `--output DIR`). Assets are picked by intent: the CLI matches its own platform archive (`--asset cli`, the default) while `--asset desktop` picks the NSIS setup exe. A plain check prints the current/latest version, the release page URL and the release notes; "update available" still exits 0 so scripts are unaffected. `--progress json` emits a third event dialect (`schema → update-result → download-start → download-progress* → download-end`, contract in docs/sidecar-events.md).

  The desktop app gains an "About & update" settings page: on-launch automatic checks (at most once a day, toggleable), manual checks, a release-notes card, and a download-and-install flow. The installer is fetched into the OS temp dir (with symlink-escape defenses) and launched only after a strict path validation; the app exits right after spawning it so the installer can overwrite the app's own files.

  Known limits: desktop installers are Windows-only (other platforms are pointed at the release page), and downloads are not hash-verified (same trust model as a manual download). ([#6](https://github.com/ONEGAYI/obsidian-export-desktop/pull/6))


## [26.8.5](https://github.com/zoni/obsidian-export/tree/26.8.5) - 2026-08-27

Patch release for the link checker: section anchors with a leading list-style marker (`5. Title`) are no longer mass-reported as broken (and embeds into such sections no longer collapse), and duplicate headings now verify their GitHub `-1` disambiguation suffixes.

### Fixes

- Section anchors with a leading list-style marker (`5. Title`, `- Title`, `> Title`) are no longer mass-reported as broken by `obsidian-export check`, and embeds pointing at such sections no longer silently collapse.

  The section query was re-parsed as block-level Markdown, which consumed the `N. ` prefix of numbered headings as an ordered-list marker and made the query never equal the target heading. The query is now parsed as heading inline content (the same parser flavor as whole-note parsing), so list/quote markers stay literal while inline formatting still aggregates to its rendered text. This also fixes embedded exports `![[note#5. Section]]` failing to slice the section.

  Duplicate headings in one document now verify their GitHub disambiguation suffixes: a fragment like `#dup-1` (targeting the second `## Dup`) is accepted by the checker, both across files and as a same-file fragment, while out-of-range suffixes (`#dup-2` for two duplicates) still report as broken. Generated export links are unchanged — a wikilink always targets the first matching heading, whose bare slug is already correct on GitHub. ([#5](https://github.com/ONEGAYI/obsidian-export-desktop/pull/5))


## [26.8.4](https://github.com/zoni/obsidian-export/tree/26.8.4) - 2026-08-24

This release wires the link checker into the desktop app — an automatic check after export (against the vault source or the exported tree) plus a paginated settings view — adds the machine-readable `check --progress json` event stream, and fixes accessibility labeling for settings switches, filter tabs, and inputs.

### New Features

- Desktop: automatic link check after export + paginated settings

  The desktop app can now run the link checker automatically after a
  successful export. A new "Link Check" settings page toggles it and picks
  the check target: the vault source (default — catches dead wikilinks,
  embeds, and plain links before conversion) or the exported tree (verifies
  the generated Markdown links and anchors; broken wikilinks have already
  collapsed to plain text there). Checking the vault source reuses the
  export's filter options so the checked file set matches the exported one.

  The report panel shows the run summary (files, links, broken, skipped)
  with filter tabs (broken by default, all, skipped); verdicts are rendered
  from the structured payloads and localized, while paths and raw link text
  stay verbatim. A check can be cancelled by going back to the main view.

  The settings view itself was rebuilt as a paginated layout (side
  navigation: Conversion / Content Filtering / Files & Process / Link
  Check), collapsing to horizontal tabs on narrow windows.

- `check --progress json`: machine-readable link-check event stream

  The `check` subcommand now accepts `--progress json`, emitting the same
  JSON Lines dialect family as exports: a `schema` header (shared version
  constant), `check-start`, one `link-report` per link with fully structured
  payloads (`source`, `line`, `raw`, `kind`, and a `status` object whose
  variants carry the target/section/block names), and a `check-end` summary
  (`filesChecked`, `totalLinks`, `broken`, `skipped`). Consumers no longer
  parse the human-readable verdict lines. The termination protocol mirrors
  exports (a run that fails after the schema line emits no `check-end`; the
  reason stays on stderr), exit codes stay 0/1/2, and the plain-text mode is
  unchanged.

### Fixes

- Desktop: settings switches and filter tabs are now fully labeled for screen readers and accessibility trees

  The settings switches (hard line breaks, hidden files, link check, …)
  exposed no accessible name at all and appeared as anonymous buttons in
  accessibility trees. Their accessible name now carries the row title plus
  the current state ("Hard line breaks (on)"): tree-based tooling typically
  does not render the toggle state, so without the state in the name a click
  left the observed tree completely unchanged. Screen readers will announce
  the state twice (name and toggle state) — an accepted trade-off for
  walk-through tooling. Additionally, the link-check report's filter tabs
  now expose their pressed state (`aria-pressed`), path inputs are named by
  their field label instead of falling back to the placeholder, and the
  radio groups carry their group label.


## [26.8.3](https://github.com/zoni/obsidian-export/tree/26.8.3) - 2026-08-24

This release adds a link-integrity checker (`obsidian-export check`) that verifies every wikilink and Markdown link in a vault — file existence, anchor/block resolution, and out-of-root escapes — and aligns generated section anchors exactly with what GitHub and VS Code produce, fixing links to headings that contain fullwidth punctuation.

### New Features

- New `check` subcommand and library API for vault link integrity

  `obsidian-export check SOURCE` (and the new `Exporter::check()` library
  method) walks the same files an export would process and verifies every
  link without writing anything:

  - Obsidian references (`[[note]]`, `[[note#section]]`, `[[note#^block]]`,
    embeds included) resolve exactly the way the exporter resolves them;
  - standard Markdown links/images must point to a file inside the checked
    root — the root is the export boundary, so links that escape it
    (`../sibling`, absolute paths, other drives) are reported as broken even
    when the file exists on disk;
  - section anchors are validated per target (Obsidian-style matching for
    wikilinks, GitHub-style slugs for markdown fragments), block ids reuse
    the exporter's block-locating rules;
  - external URLs (`https://…`) are skipped and counted separately.

  Output is one line per link, `{source}:{line}: {status} [{raw}]`, plus a
  summary; the exit code stays within the documented 0/1/2 contract (any
  broken link exits 1). The desktop app can later run this automatically
  after an export (configuration point pending).

  Note for scripts: a leading `check` argument is now always treated as the
  subcommand. Exporting from a vault folder that happens to be named `check`
  now requires spelling it `./check`; the CLI prints a warning when it
  detects that situation.

  ([#link-check](https://github.com/zoni/obsidian-export/issues/link-check))

### Fixes

- NSIS installer now shows the app icon explicitly

  The Windows NSIS setup wizard no longer falls back to a generic executable
  icon; the installer configuration now sets the application icon explicitly,
  so shortcuts and Add/Remove Programs entries pick up the Obsidian Export
  icon. ([#nsis-icon](https://github.com/zoni/obsidian-export/issues/nsis-icon))
- Section anchors now match exactly what GitHub and VS Code generate for a heading

  Anchors are now produced with the `github-slugger` crate instead of a hand-rolled
  slug function, fixing four divergences from GitHub/VS Code behavior (vectors for
  the new behavior were captured from live GitHub rendering):

  - Fullwidth punctuation such as `：` and `，` is now stripped. Previously it was
    kept verbatim in the anchor, so links like `[[note#总纲：三份形态，两个断口]]`
    produced `#总纲：三份形态，两个断口`, which resolves on neither GitHub nor VS
    Code (both generate `#总纲三份形态两个断口`).
  - Punctuation no longer leaves a hyphen behind. Numbered headings such as
    `1.1.1 C` (see the upstream issue this closes for the "Number Headings"
    plugin use case) now produce `#111-c` instead of the broken `#1-1-1-c`.
  - Runs of consecutive hyphens are kept (`this--or-that` stays `this--or-that`).
  - Leading/trailing hyphens are no longer trimmed.

  Link display text is unchanged; only the `#anchor` part of generated links is
  affected.

  ([#370](https://github.com/zoni/obsidian-export/issues/370))


## [26.8.2](https://github.com/zoni/obsidian-export/tree/26.8.2) - 2026-08-22

This release makes the desktop app bilingual — every UI string moved into i18n dictionaries with an English translation alongside the original Chinese, switched through a title-bar language menu (Chinese / English / follow system) — and restores section references to headings that contain wikilinks: `![[note#mid]]` embeds a `## [[mid]]` heading again.

### New Features

- Desktop: bilingual UI (Chinese / English) with a language menu

  All desktop UI strings moved from hard-coded Chinese into i18n dictionaries, with a full English translation alongside. A language dropdown in the title bar offers Chinese, English, and "follow system" (detected from the OS locale, any `zh*` locale resolves to Chinese); any of the three can be picked freely and the choice persists across sessions.

### Fixes

- Section references resolve headings that contain wikilinks again

  A heading like `## [[mid]]` now aggregates by its display text ("mid") when matching section references, so `![[note#mid]]` embeds that section again — the behavior that existed before the raw-parse/expand split. Inside the embedded slice the wikilink itself still expands into a normal link. Literal single-bracket headings (e.g. `## [WIP] Notes`) keep aggregating verbatim, and mixed headings (`## [WIP] and [[mid]]`) treat both parts correctly. Nested spellings inside a reference (`![[note#[[mid]]]]`) remain unsupported: the wikilink grammar itself forbids `]` inside reference text.


## [26.8.1](https://github.com/zoni/obsidian-export/tree/26.8.1) - 2026-08-22

This release migrates the YAML dependency to the YAML-org-maintained `yaml_serde` (API-compatible; MSRV is now 1.82), adds a full conversion options view to the desktop app, and lands a parsing-layer correctness pass: block references now resolve to the blocks they mark, same-file embeds are supported, section cuts happen before nested embeds expand, blockquote-contained headings no longer corrupt output, and wikilinks keep their original formatting spellings.

### New Features

- Block references now resolve to the block they mark: `![[note#^block-id]]`
  embeds the marked paragraph, list item or quote block (an id alone on its
  own line marks the block above it). The id marker is stripped from the
  embedded copy; ids that don't resolve fall back to the `--missing-section`
  strategy. Same-file section and block embeds (`![[#Heading]]` /
  `![[#^block-id]]`) are now supported as well — previously they degraded
  to plain links. ([#block-references](https://github.com/zoni/obsidian-export/issues/block-references))
- Desktop: added a full conversion options view exposing every CLI flag of
  the sidecar (frontmatter strategy, missing-section handling, hard line
  breaks, recursive embeds, hidden files, git integration, ignore file name,
  skip/only tags, start-at sub-path, preserve mtime, fail-fast). Options are
  persisted across sessions and only non-default values are forwarded to the
  CLI. The pre-export dialog now summarizes the effective options with a
  shortcut to edit them, and picked paths are normalized to absolute form
  per the sidecar contract. ([#desktop-options-panel](https://github.com/zoni/obsidian-export/issues/desktop-options-panel))

### Changes

- Section cuts on embeds now happen on the embedded note's own events,
  before its nested embeds are expanded. Previously a heading pulled in by
  a nested embed could terminate the outer section cut early, silently
  dropping the outer note's own content after it. The embed postprocessor
  contract (postprocessors see fully expanded content) is unchanged. ([#embed-section-cut-order](https://github.com/zoni/obsidian-export/issues/embed-section-cut-order))
- Migrated the YAML dependency from the archived `serde_yaml 0.9.34` to
  `yaml_serde 0.10` (maintained by the YAML organization) via a Cargo package
  rename. The public `obsidian_export::serde_yaml` path, the `Frontmatter`
  type alias and all parsing/emitting behavior are unchanged. The minimum
  supported Rust version was bumped from 1.80 to 1.82 as required by
  yaml_serde. ([#yaml-serde-migration](https://github.com/zoni/obsidian-export/issues/yaml-serde-migration))

### Fixes

- Section cuts around headings inside block containers (blockquotes, lists)
  no longer produce unbalanced event streams. Previously the stray
  Start/End events polluted the renderer's padding stack and could swallow
  following output into the quote — e.g. an Obsidian callout heading
  referenced via `![[note#Heading]]`. ([#container-balanced-section-cut](https://github.com/zoni/obsidian-export/issues/container-balanced-section-cut))
- Wikilink reference text now keeps its original spelling: `[[b#__dunder__]]`
  no longer mutates into `b#**dunder**`. Anchors keep underscores (matching
  GitHub-style heading anchors), filenames like `[[__note__]]` resolve
  again, and section queries with formatting markers (`*Target* Heading`)
  match the headings they render to. ([#wikilink-spelling-preserved](https://github.com/zoni/obsidian-export/issues/wikilink-spelling-preserved))


## [26.8.0](https://github.com/zoni/obsidian-export/tree/26.8.0) - 2026-08-22

This release ships a Tauri-based desktop GUI on top of the CLI, adds a machine-readable `--progress json` event stream with a `--missing-section` strategy option, and lands a broad correctness pass over wikilink resolution and link generation (relative `../` references, Windows path separators, non-ASCII destinations). The default missing-section behavior is now `skip`; several failure modes now surface as per-file errors instead of aborting or silently misbehaving.

### New Features

- Add a desktop GUI (Tauri 2 sidecar app)

  A graphical desktop app now ships alongside the CLI, turning the export flow into pick-paths → confirm → watch progress. The GUI never implements conversion itself: it bundles the CLI as a sidecar process and consumes its `--progress json` event stream (contract in `docs/sidecar-events.md`).

  Highlights:

  - Obsidian-styled interface (light/dark/follow-system themes), frameless window with a custom title bar.
  - Path pickers with remembered last-used locations.
  - A pre-export confirmation sheet for the missing-section strategy and an option to export into `<destination>/<vault folder name>` so first-level files don't scatter.
  - Live progress, colored per-file log lines, failure details with full error chains, and cancellation.

  The CLI remains fully usable standalone; the desktop app is just another way to invoke it. See `docs/BUILD.md` for building and running the desktop app.
- Machine-readable progress events with `--progress json`

  Passing `--progress json` emits progress events on stdout as JSON Lines (one JSON object per line), intended for programs driving obsidian-export as a child process. The stream starts with a schema-version line, followed by per-file progress, warnings (with the originating file), and a terminating end event listing failed files. Without the flag, stdout stays silent as before.

### Fixes

- Support markdown formatting in wikilinks

  Previously, links with formatting such as bold or italics (like `[[Note|Example **bold** and *italic* link text]]`) were not accounted for correctly, resulting in such links being rendered as literal text instead.
  Now these will parse correctly and render actual links as intended. ([#329](https://github.com/zoni/obsidian-export/issues/329))
- Fix `--preserve-mtime` error when files are skipped by postprocessors

  When `--preserve-mtime` is enabled and files are skipped by postprocessors (e.g., due to `--skip-tags`), the exporter would attempt to set the modification time on non-existent destination files, causing an error.

  Now this no longer happens because the exporter won't attempt to set an mtime when postprocessors caused files to be skipped. ([#348](https://github.com/zoni/obsidian-export/issues/348))
- Fix CLI error handling and argument edge cases

  - `--help` output now goes to stdout (exit code 0) instead of stderr; argument errors exit with code 2 while runtime errors exit with code 1.
  - Non-UTF-8 command-line arguments no longer panic the process.
  - A `--start-at` path outside the export root now fails with a clear error instead of silently exporting zero files.
  - Unicode heading anchors are preserved as-is instead of being transliterated (e.g. Chinese headings used to become pinyin, producing anchors no renderer matches), and underscores in anchors are now kept, matching GitHub's slug rules.
  - Degenerate wikilinks like `[[note|]]` or `[[#]]` no longer panic the export.
  - Resolution of bare-name references to same-named files is now deterministic (fewest path components, then lexicographic order) instead of depending on directory traversal order.
  - `filter_by_tags` now also accepts scalar and comma-separated string values for `tags` in frontmatter.
  - Closing the stdout pipe (e.g. a consumer that stopped reading `--progress json` output) exits quietly with code 1 instead of panicking with code 101.
- Fix section matching for headings with inline code and nested same-named headings

  Headings containing inline code or math (e.g. `## \`code\` heading`) failed to match section references, since only plain text events were aggregated into the heading name. Their literal text now counts towards the heading name.

  Additionally, a same-named heading nested deeper than the target no longer restarts the section: `![[note#Target]]` now embeds from the first matching heading to the end of that section, instead of just the innermost part.
- Keep non-ASCII characters verbatim in link destinations

  Link destinations percent-encoded every non-ASCII character (e.g. `图.svg` became `%E5%9B%BE.svg`), making exported notes hard to read and diff. Only characters that would break a Markdown inline link destination or URL semantics (controls, spaces, parentheses, `%`, `?`, `#`) are escaped now; filenames in Chinese or other non-ASCII scripts stay readable, matching what Obsidian itself writes.
- Resolve wikilinks with explicit relative components (`./`, `../`)

  Wikilinks such as `![[../assets/diagram.svg]]` were silently dropped (with a warning) because vault lookup only matches path suffixes, which can never contain `.` or `..` components. Obsidian resolves such references against the containing note's directory, and so does the exporter now: the reference is normalized against the note's location before lookup. References that would escape the vault root remain unresolved.
- Use forward slashes in generated link destinations on Windows

  Relative link destinations generated on Windows contained backslashes (e.g. `![img](..\assets\img.png)`), which most Markdown renderers cannot resolve. Destinations now always use forward slashes, matching the output on Unix platforms.

### Backwards-incompatible Changes

- Failing notes no longer abort the export by default

  Previously the first note that failed to export (e.g. broken YAML frontmatter) aborted the whole run. Now failures are collected per note, the export continues with the remaining notes, and a summary listing every failing note is printed at the end.

  Pass `--fail-fast` to restore the old stop-on-first-failure behavior.
- Missing sections in embeds no longer silently embed the full note

  When an embed pointed at a section (heading) that doesn't exist in the target note — including block references like `![[note#^block-id]]` — the entire note used to be embedded silently, containing more content than the reference asked for.

  By default such an embed now collapses to nothing and a warning is emitted, matching Obsidian's own "not found" rendering. The previous behavior remains available as `--missing-section embed-full`; `--missing-section fail` turns it into an error instead.


## [25.3.0](https://github.com/zoni/obsidian-export/tree/25.3.0) - 2025-03-25

### Changes

- Support Github-Flavored-Markdown

  The Github-Flavored-Markdown extension is now enabled in the markdown parser.
  This ensures Obsidian callouts don't end up mangled by having escaping added to them. ([#328](https://github.com/zoni/obsidian-export/issues/328), [#330](https://github.com/zoni/obsidian-export/issues/330))

### Miscellaneous

- Dependency updates


## [24.11.0](https://github.com/zoni/obsidian-export/tree/24.11.0) - 2024-11-23

### New Features

- Optionally preserve modified time of exported files

  Add a new argument `--preserve-mtime` to keep the original modified time attribute of notes being exported, instead of setting them to the current time.

  Contribution made by [Davis Davalos-DeLosh](https://github.com/Programmerino). ([#154](https://github.com/zoni/obsidian-export/issues/154), [#204](https://github.com/zoni/obsidian-export/issues/204))

### Changes

- Bump to the minimum supported Rust version to 1.80.0

  Obsidian-export now uses [std::sync::LazyLock](https://doc.rust-lang.org/std/sync/struct.LazyLock.html) instead of [lazy_static](https://crates.io/crates/lazy_static), which was only stabilized in Rust 1.80.0.
  This change made it possible to drop the external dependency on lazy_static, though as a result of this, compiling with older versions will no longer be possible.

### Fixes

- Don't escape square brackets in math expressions

  The upgrade to [pulldown-cmark](https://crates.io/crates/pulldown-cmark) 0.11 (see Backwards-incompatible Changes) includes official support for LaTeX-style math expressions.
  With the markdown parser supporting this syntax natively, math expressions are now processed correctly without edge-cases. ([#14](https://github.com/zoni/obsidian-export/issues/14), [#252](https://github.com/zoni/obsidian-export/issues/252))

### Backwards-incompatible Changes

- Upgrade [pulldown-cmark](https://crates.io/crates/pulldown-cmark) from 0.9 to 0.12

  pulldown-cmark is the Markdown/CommonMark parser that is used to read and convert notes (together with [pulldown-cmark-to-cmark](https://crates.io/crates/pulldown-cmark-to-cmark)).

  For end-users that call the obsidian-export CLI this upgrade will be mostly transparent, except that Math blocks are now properly processed without getting mangled.

  People who use the library directly may face more significant breaking changes if they have custom postprocessors, as pulldown-cmark's events have gone through various breaking changes.
  For more information, see:

  - <https://github.com/zoni/obsidian-export/pull/252>
  - <https://github.com/pulldown-cmark/pulldown-cmark/releases/tag/v0.10.0>
  - <https://github.com/zoni/obsidian-export/pull/276/files#diff-b1a35a68f14e696205874893c07fd24fdb88882b47c23cc0e0c80a30c7d53759>

  ([#14](https://github.com/zoni/obsidian-export/issues/14), [#252](https://github.com/zoni/obsidian-export/issues/252), [#259](https://github.com/zoni/obsidian-export/issues/259), [#285](https://github.com/zoni/obsidian-export/issues/285))


## v23.12.0 (2023-12-03)

### New

- Implement frontmatter based filtering (#163) [Martin Heuschober]

  This allows limiting the notes that will be exported using `--skip-tags` and `--only-tags`:

  - using `--skip-tags foo --skip-tags bar` will skip any files that have the tags `foo` or `bar` in their frontmatter
  - using `--only-tags foo --only-tags bar` will skip any files that **don't** have the tags `foo` or `bar` in their frontmatter

### Fixes

- Trim filenames while resolving wikilinks [Nick Groenen]

  Obsidian trims the filename part in a [[WikiLink|label]], so each of
  these are equivalent:

  ```
  [[wikilink]]
  [[ wikilink ]]
  [[ wikilink |wikilink]]
  ```

  Obsidian-export now behaves similarly.

  Fixes #188

### Other

- Relicense to BSD-2-Clause Plus Patent License [Nick Groenen]

  This license achieves everything that dual-licensing under MIT + Apache
  aims for, but without the weirdness of being under two licenses.

  Having checked external contributions, I feel pretty confident that I
  can unilaterally make this license change, as people have only
  contributed a handful of one-line changes of no significance towards
  copyrighted work up to this point.


- Add a lifetime annotation to the Postprocesor type [Robert Sesek]

  This lets the compiler reason about the lifetimes of objects used by the
  postprocessor, if the callback captures variables.

  See zoni/obsidian-export#175

- Use cargo-dist to create release artifacts [Nick Groenen]

  This will create binaries for more platforms (including ARM builds for
  MacOS) and installer scripts in addition to just the binaries themselves.

## v22.11.0 (2022-11-19)

### New

* Apply unicode normalization while resolving notes. [Nick Groenen]

  The unicode standard allows for certain (visually) identical characters to
  be represented in different ways.

  For example the character ä may be represented as a single combined
  codepoint "Latin Small Letter A with Diaeresis" (U+00E4) or by the
  combination of "Latin Small Letter A" (U+0061) followed by "Combining
  Diaeresis" (U+0308).

  When encoded with UTF-8, these are represented as respectively the two
  bytes 0xC3 0xA4, and the three bytes 0x61 0xCC 0x88.

  A user linking to notes with these characters in their titles would
  expect these two variants to link to the same file, given they are
  visually identical and have the exact same semantic meaning.

  The unicode standard defines a method to deconstruct and normalize these
  forms, so that a byte comparison on the normalized forms of these
  variants ends up comparing the same thing. This is called Unicode
  Normalization, defined in Unicode® Standard Annex #15
  (http://www.unicode.org/reports/tr15/).

  The W3C Working Group has written an excellent explanation of the
  problems regarding string matching, and how unicode normalization helps
  with this process: https://www.w3.org/TR/charmod-norm/#unicodeNormalization

  With this change, obsidian-export will perform unicode normalization
  (specifically the C (or NFC) normalization form) on all note titles
  while looking up link references, ensuring visually identical links are
  treated as being similar, even if they were encoded as different
  variants.

  A special thanks to Hans Raaf (@oderwat) for reporting and helping track
  down this issue.

### Breaking Changes (affects library API only)

* Pass context and events as mutable references to postprocessors. [Nick Groenen]

  Instead of passing clones of context and the markdown tree to
  postprocessors, pass them a mutable reference which may be modified
  in-place.

  This is a breaking change to the postprocessor implementation, changing
  both the input arguments as well as the return value:

  ```diff
  -    dyn Fn(Context, MarkdownEvents) -> (Context, MarkdownEvents, PostprocessorResult) + Send + Sync;
  +    dyn Fn(&mut Context, &mut MarkdownEvents) -> PostprocessorResult + Send + Sync;
  ```

  With this change the postprocessor API becomes a little more ergonomic
  to use however, especially making the intent around return statements more clear.

### Other

* Use path.Join to construct hugo links (#92) [Chang-Yen Tseng]

  Use path.Join so that it will render correctly on Windows
  (path.Join will convert Windows backslash to forward slash)

* Bump crossbeam-utils from 0.8.5 to 0.8.12. [dependabot[bot]]

  Bumps [crossbeam-utils](https://github.com/crossbeam-rs/crossbeam) from 0.8.5 to 0.8.12.
  - [Release notes](https://github.com/crossbeam-rs/crossbeam/releases)
  - [Changelog](https://github.com/crossbeam-rs/crossbeam/blob/master/CHANGELOG.md)
  - [Commits](https://github.com/crossbeam-rs/crossbeam/compare/crossbeam-utils-0.8.5...crossbeam-utils-0.8.12)

  ---
  updated-dependencies:
  - dependency-name: crossbeam-utils
    dependency-type: indirect
  ...

* Bump regex from 1.6.0 to 1.7.0. [dependabot[bot]]

  Bumps [regex](https://github.com/rust-lang/regex) from 1.6.0 to 1.7.0.
  - [Release notes](https://github.com/rust-lang/regex/releases)
  - [Changelog](https://github.com/rust-lang/regex/blob/master/CHANGELOG.md)
  - [Commits](https://github.com/rust-lang/regex/compare/1.6.0...1.7.0)

  ---
  updated-dependencies:
  - dependency-name: regex
    dependency-type: direct:production
    update-type: version-update:semver-minor
  ...

* Bump actions/checkout from 2 to 3. [dependabot[bot]]

  Bumps [actions/checkout](https://github.com/actions/checkout) from 2 to 3.
  - [Release notes](https://github.com/actions/checkout/releases)
  - [Changelog](https://github.com/actions/checkout/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/actions/checkout/compare/v2...v3)

  ---
  updated-dependencies:
  - dependency-name: actions/checkout
    dependency-type: direct:production
    update-type: version-update:semver-major
  ...

* Bump actions/upload-artifact from 2 to 3. [dependabot[bot]]

  Bumps [actions/upload-artifact](https://github.com/actions/upload-artifact) from 2 to 3.
  - [Release notes](https://github.com/actions/upload-artifact/releases)
  - [Commits](https://github.com/actions/upload-artifact/compare/v2...v3)

  ---
  updated-dependencies:
  - dependency-name: actions/upload-artifact
    dependency-type: direct:production
    update-type: version-update:semver-major
  ...

* Bump thread_local from 1.1.3 to 1.1.4. [dependabot[bot]]

  Bumps [thread_local](https://github.com/Amanieu/thread_local-rs) from 1.1.3 to 1.1.4.
  - [Release notes](https://github.com/Amanieu/thread_local-rs/releases)
  - [Commits](https://github.com/Amanieu/thread_local-rs/compare/v1.1.3...1.1.4)

  ---
  updated-dependencies:
  - dependency-name: thread_local
    dependency-type: indirect
  ...

* Remove needless borrows. [Nick Groenen]

* Upgrade snafu to 0.7.x. [Nick Groenen]

* Upgrade pulldown-cmark-to-cmark to 10.0.x. [Nick Groenen]

* Upgrade serde_yaml to 0.9.x. [Nick Groenen]

* Upgrade minor dependencies. [Nick Groenen]

* Fix new clippy lints. [Nick Groenen]

* Add a contributor guide. [Nick Groenen]

* Simplify pre-commit setup. [Nick Groenen]

  No need to depend on a third-party hook repository when each of these
  checks is easily defined and run through system commands.

  This also allows us to actually run tests, which is current unsupported
  (https://github.com/doublify/pre-commit-rust/pull/19)

* Bump tempfile from 3.2.0 to 3.3.0. [dependabot[bot]]

  Bumps [tempfile](https://github.com/Stebalien/tempfile) from 3.2.0 to 3.3.0.
  - [Release notes](https://github.com/Stebalien/tempfile/releases)
  - [Changelog](https://github.com/Stebalien/tempfile/blob/master/NEWS)
  - [Commits](https://github.com/Stebalien/tempfile/compare/v3.2.0...v3.3.0)

  ---
  updated-dependencies:
  - dependency-name: tempfile
    dependency-type: direct:production
    update-type: version-update:semver-minor
  ...

## v22.1.0 (2022-01-02)

Happy new year! On this second day of 2022 comes a fresh release with one
notable new feature.

### New

* Support Obsidian's "Strict line breaks" setting. [Nick Groenen]

  This change introduces a new `--hard-linebreaks` CLI argument. When
  used, this converts soft line breaks to hard line breaks, mimicking
  Obsidian's "Strict line breaks" setting.

  > Implementation detail: I considered naming this flag
  > `--strict-line-breaks` to be consistent with Obsidian itself, however I
  > feel the name is somewhat misleading and ill-chosen.

### Other

* Give release binaries file extensions. [Nick Groenen]

  This may make it more clear to users that these are precompiled, binary
  files. This is especially relevant on Windows, where the convention is
  that executable files have a `.exe` extension, as seen in #49.

* Upgrade dependencies. [Nick Groenen]

  This commit upgrades all dependencies to their current latest versions. Most
  notably, this includes upgrades to the following most critical libraries:

      pulldown-cmark v0.8.0 -> v0.9.0
      pulldown-cmark-to-cmark v7.1.1 -> v9.0.0

  In total, these dependencies were upgraded:

      bstr v0.2.16 -> v0.2.17
      ignore v0.4.17 -> v0.4.18
      libc v0.2.101 -> v0.2.112
      memoffset v0.6.4 -> v0.6.5
      num_cpus v1.13.0 -> v1.13.1
      once_cell v1.8.0 -> v1.9.0
      ppv-lite86 v0.2.10 -> v0.2.16
      proc-macro2 v1.0.29 -> v1.0.36
      pulldown-cmark v0.8.0 -> v0.9.0
      pulldown-cmark-to-cmark v7.1.1 -> v9.0.0
      quote v1.0.9 -> v1.0.14
      rayon v1.5.0 -> v1.5.1
      regex v1.5.3 -> v1.5.4
      serde v1.0.130 -> v1.0.132
      syn v1.0.75 -> v1.0.84
      unicode-width v0.1.8 -> v0.1.9
      version_check v0.9.3 -> v0.9.4

* Bump serde_yaml from 0.8.21 to 0.8.23 (#52) [dependabot[bot]]

  Bumps [serde_yaml](https://github.com/dtolnay/serde-yaml) from 0.8.21 to 0.8.23.
  - [Release notes](https://github.com/dtolnay/serde-yaml/releases)
  - [Commits](https://github.com/dtolnay/serde-yaml/compare/0.8.21...0.8.23)

  ---
  updated-dependencies:
  - dependency-name: serde_yaml
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Bump pulldown-cmark-to-cmark from 7.1.0 to 7.1.1 (#51) [dependabot[bot]]

  Bumps [pulldown-cmark-to-cmark](https://github.com/Byron/pulldown-cmark-to-cmark) from 7.1.0 to 7.1.1.
  - [Release notes](https://github.com/Byron/pulldown-cmark-to-cmark/releases)
  - [Changelog](https://github.com/Byron/pulldown-cmark-to-cmark/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/Byron/pulldown-cmark-to-cmark/compare/v7.1.0...v7.1.1)

  ---
  updated-dependencies:
  - dependency-name: pulldown-cmark-to-cmark
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Bump pulldown-cmark-to-cmark from 7.0.0 to 7.1.0 (#48) [dependabot[bot]]

  Bumps [pulldown-cmark-to-cmark](https://github.com/Byron/pulldown-cmark-to-cmark) from 7.0.0 to 7.1.0.
  - [Release notes](https://github.com/Byron/pulldown-cmark-to-cmark/releases)
  - [Changelog](https://github.com/Byron/pulldown-cmark-to-cmark/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/Byron/pulldown-cmark-to-cmark/compare/v7.0.0...v7.1.0)

  ---
  updated-dependencies:
  - dependency-name: pulldown-cmark-to-cmark
    dependency-type: direct:production
    update-type: version-update:semver-minor
  ...

* Bump pulldown-cmark-to-cmark from 6.0.4 to 7.0.0 (#47) [dependabot[bot]]

  Bumps [pulldown-cmark-to-cmark](https://github.com/Byron/pulldown-cmark-to-cmark) from 6.0.4 to 7.0.0.
  - [Release notes](https://github.com/Byron/pulldown-cmark-to-cmark/releases)
  - [Changelog](https://github.com/Byron/pulldown-cmark-to-cmark/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/Byron/pulldown-cmark-to-cmark/compare/v6.0.4...v7.0.0)

  ---
  updated-dependencies:
  - dependency-name: pulldown-cmark-to-cmark
    dependency-type: direct:production
    update-type: version-update:semver-major
  ...

* Bump pathdiff from 0.2.0 to 0.2.1 (#46) [dependabot[bot]]

  Bumps [pathdiff](https://github.com/Manishearth/pathdiff) from 0.2.0 to 0.2.1.
  - [Release notes](https://github.com/Manishearth/pathdiff/releases)
  - [Commits](https://github.com/Manishearth/pathdiff/commits)

  ---
  updated-dependencies:
  - dependency-name: pathdiff
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Bump pulldown-cmark-to-cmark from 6.0.3 to 6.0.4 (#44) [dependabot[bot]]

  Bumps [pulldown-cmark-to-cmark](https://github.com/Byron/pulldown-cmark-to-cmark) from 6.0.3 to 6.0.4.
  - [Release notes](https://github.com/Byron/pulldown-cmark-to-cmark/releases)
  - [Changelog](https://github.com/Byron/pulldown-cmark-to-cmark/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/Byron/pulldown-cmark-to-cmark/compare/v6.0.3...v6.0.4)

  ---
  updated-dependencies:
  - dependency-name: pulldown-cmark-to-cmark
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Bump pretty_assertions from 0.7.2 to 1.0.0 (#45) [dependabot[bot]]

  Bumps [pretty_assertions](https://github.com/colin-kiegel/rust-pretty-assertions) from 0.7.2 to 1.0.0.
  - [Release notes](https://github.com/colin-kiegel/rust-pretty-assertions/releases)
  - [Changelog](https://github.com/colin-kiegel/rust-pretty-assertions/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/colin-kiegel/rust-pretty-assertions/compare/v0.7.2...v1.0.0)

  ---
  updated-dependencies:
  - dependency-name: pretty_assertions
    dependency-type: direct:production
    update-type: version-update:semver-major
  ...

## v21.9.1 (2021-09-24)

### Changes

* Treat SVG files as embeddable images. [Narayan Sainaney]

  This will ensure SVG files are included as an image when using `![[foo.svg]]` syntax, as opposed to only being linked to.

### Other

* Bump pulldown-cmark-to-cmark from 6.0.2 to 6.0.3. [dependabot[bot]]

  Bumps [pulldown-cmark-to-cmark](https://github.com/Byron/pulldown-cmark-to-cmark) from 6.0.2 to 6.0.3.
  - [Release notes](https://github.com/Byron/pulldown-cmark-to-cmark/releases)
  - [Changelog](https://github.com/Byron/pulldown-cmark-to-cmark/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/Byron/pulldown-cmark-to-cmark/compare/v6.0.2...v6.0.3)

  ---
  updated-dependencies:
  - dependency-name: pulldown-cmark-to-cmark
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Bump serde_yaml from 0.8.20 to 0.8.21. [dependabot[bot]]

  Bumps [serde_yaml](https://github.com/dtolnay/serde-yaml) from 0.8.20 to 0.8.21.
  - [Release notes](https://github.com/dtolnay/serde-yaml/releases)
  - [Commits](https://github.com/dtolnay/serde-yaml/compare/0.8.20...0.8.21)

  ---
  updated-dependencies:
  - dependency-name: serde_yaml
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...



## v21.9.0 (2021-09-12)

> This release switches to a [calendar versioning scheme](https://calver.org/overview.html).
> Details on this decision can be read in [switching obsidian-export to CalVer](https://nick.groenen.me/posts/switching-obsidian-export-to-calver/).

### New

* Support postprocessors running on embedded notes. [Nick Groenen]

  This introduces support for postprocessors that are run on the result of
  a note that is being embedded into another note. This differs from the
  existing postprocessors (which remain unchanged) that run once all
  embeds have been processed and merged with the final note.

  These "embed postprocessors" may be set through the new
  `Exporter::add_embed_postprocessor` method.

* Add start_at option to export a partial vault. [Nick Groenen]

  This introduces a new `--start-at` CLI argument and corresponding
  `start_at()` method on the Exporter type that allows exporting of only a
  given subdirectory within a vault.

  See the updated README file for more details on when and how this may be
  used.

### Other

* Don't build docs for the bin target. [Nick Groenen]

  The library contains documentation covering both CLI and library usage,
  there's no separate documentation for just the binary target.

* Move postprocessor tests into their own file for clarity. [Nick Groenen]

* Update indirect dependencies. [Nick Groenen]

* Bump serde_yaml from 0.8.19 to 0.8.20. [dependabot[bot]]

  Bumps [serde_yaml](https://github.com/dtolnay/serde-yaml) from 0.8.19 to 0.8.20.
  - [Release notes](https://github.com/dtolnay/serde-yaml/releases)
  - [Commits](https://github.com/dtolnay/serde-yaml/compare/0.8.19...0.8.20)

  ---
  updated-dependencies:
  - dependency-name: serde_yaml
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Don't borrow references that are immediately dereferenced. [Nick Groenen]

  This was caught by a recently introduced clippy rule

* Bump serde_yaml from 0.8.17 to 0.8.19. [dependabot[bot]]

  Bumps [serde_yaml](https://github.com/dtolnay/serde-yaml) from 0.8.17 to 0.8.19.
  - [Release notes](https://github.com/dtolnay/serde-yaml/releases)
  - [Commits](https://github.com/dtolnay/serde-yaml/compare/0.8.17...0.8.19)

  ---
  updated-dependencies:
  - dependency-name: serde_yaml
    dependency-type: direct:production
    update-type: version-update:semver-patch
  ...

* Update dependencies. [Nick Groenen]

* Fix 4 new clippy lints. [Nick Groenen]

* Bump regex from 1.4.6 to 1.5.3. [dependabot[bot]]

  Bumps [regex](https://github.com/rust-lang/regex) from 1.4.6 to 1.5.3.
  - [Release notes](https://github.com/rust-lang/regex/releases)
  - [Changelog](https://github.com/rust-lang/regex/blob/master/CHANGELOG.md)
  - [Commits](https://github.com/rust-lang/regex/compare/1.4.6...1.5.3)

* Bump pretty_assertions from 0.7.1 to 0.7.2. [dependabot[bot]]

  Bumps [pretty_assertions](https://github.com/colin-kiegel/rust-pretty-assertions) from 0.7.1 to 0.7.2.
  - [Release notes](https://github.com/colin-kiegel/rust-pretty-assertions/releases)
  - [Changelog](https://github.com/colin-kiegel/rust-pretty-assertions/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/colin-kiegel/rust-pretty-assertions/compare/v0.7.1...v0.7.2)

* Bump regex from 1.4.5 to 1.4.6. [dependabot[bot]]

  Bumps [regex](https://github.com/rust-lang/regex) from 1.4.5 to 1.4.6.
  - [Release notes](https://github.com/rust-lang/regex/releases)
  - [Changelog](https://github.com/rust-lang/regex/blob/master/CHANGELOG.md)
  - [Commits](https://github.com/rust-lang/regex/compare/1.4.5...1.4.6)

## v0.7.0 (2021-04-11)

### New

* Postprocessing support. [Nick Groenen]

  Add support for postprocessing of Markdown prior to writing converted
  notes to disk.

  Postprocessors may be used when making use of Obsidian export as a Rust
  library to do the following:

  1. Modify a note's `Context`, for example to change the destination
     filename or update its Frontmatter.
  2. Change a note's contents by altering `MarkdownEvents`.
  3. Prevent later postprocessors from running or cause a note to be
     skipped entirely.

  Future releases of Obsidian export may come with built-in postprocessors
  for users of the command-line tool to use, if general use-cases can be
  identified.

  For example, a future release might include functionality to make notes
  more suitable for the Hugo static site generator. This functionality
  would be implemented as a postprocessor that could be enabled through
  command-line flags.

### Fixes

* Also percent-encode `?` in filenames. [Nick Groenen]

  A recent Obsidian update expanded the list of allowed characters in
  filenames, which now includes `?` as well. This needs to be
  percent-encoded for proper links in static site generators like Hugo.

### Other

* Bump pretty_assertions from 0.6.1 to 0.7.1. [dependabot[bot]]

  Bumps [pretty_assertions](https://github.com/colin-kiegel/rust-pretty-assertions) from 0.6.1 to 0.7.1.
  - [Release notes](https://github.com/colin-kiegel/rust-pretty-assertions/releases)
  - [Changelog](https://github.com/colin-kiegel/rust-pretty-assertions/blob/main/CHANGELOG.md)
  - [Commits](https://github.com/colin-kiegel/rust-pretty-assertions/compare/v0.6.1...v0.7.1)

* Bump walkdir from 2.3.1 to 2.3.2. [dependabot[bot]]

  Bumps [walkdir](https://github.com/BurntSushi/walkdir) from 2.3.1 to 2.3.2.
  - [Release notes](https://github.com/BurntSushi/walkdir/releases)
  - [Commits](https://github.com/BurntSushi/walkdir/compare/2.3.1...2.3.2)

* Bump regex from 1.4.3 to 1.4.5. [dependabot[bot]]

  Bumps [regex](https://github.com/rust-lang/regex) from 1.4.3 to 1.4.5.
  - [Release notes](https://github.com/rust-lang/regex/releases)
  - [Changelog](https://github.com/rust-lang/regex/blob/master/CHANGELOG.md)
  - [Commits](https://github.com/rust-lang/regex/compare/1.4.3...1.4.5)

## v0.6.0 (2021-02-15)

### New

* Add `--version` flag. [Nick Groenen]

### Changes

* Don't Box FilterFn in WalkOptions. [Nick Groenen]

  Previously, `filter_fn` on the `WalkOptions` struct looked like:

      pub filter_fn: Option<Box<&'static FilterFn>>,

  This boxing was unneccesary and has been changed to:

      pub filter_fn: Option<&'static FilterFn>,

  This will only affect people who use obsidian-export as a library in
  other Rust programs, not users of the CLI.

  For those library users, they no longer need to supply `FilterFn`
  wrapped in a Box.

### Fixes

* Recognize notes beginning with underscores. [Nick Groenen]

  Notes with an underscore would fail to be recognized within Obsidian
  `[[_WikiLinks]]` due to the assumption that the underlying Markdown
  parser (pulldown_cmark) would emit the text between `[[` and `]]` as
  a single event.

  The note parser has now been rewritten to use a more reliable state
  machine which correctly recognizes this corner-case (and likely some
  others).

* Support self-references. [Joshua Coles]

  This ensures links to headings within the same note (`[[#Heading]]`)
  resolve correctly.

### Other

* Avoid redundant "Release" in GitHub release titles. [Nick Groenen]

* Add failing testcase for files with underscores. [Nick Groenen]

* Add unit tests for display of ObsidianNoteReference. [Nick Groenen]

* Add some unit tests for ObsidianNoteReference::from_str. [Nick Groenen]

* Also run tests on pull requests. [Nick Groenen]

* Apply clippy suggestions following rust 1.50.0. [Nick Groenen]

* Fix infinite recursion bug with references to current file. [Joshua Coles]

* Add tests for self-references. [Joshua Coles]

  Note as there is no support for block references at the moment, the generated link goes nowhere, however it is to a reasonable ID

* Bump tempfile from 3.1.0 to 3.2.0. [dependabot[bot]]

  Bumps [tempfile](https://github.com/Stebalien/tempfile) from 3.1.0 to 3.2.0.
  - [Release notes](https://github.com/Stebalien/tempfile/releases)
  - [Changelog](https://github.com/Stebalien/tempfile/blob/master/NEWS)
  - [Commits](https://github.com/Stebalien/tempfile/commits)

* Bump eyre from 0.6.3 to 0.6.5. [dependabot[bot]]

  Bumps [eyre](https://github.com/yaahc/eyre) from 0.6.3 to 0.6.5.
  - [Release notes](https://github.com/yaahc/eyre/releases)
  - [Changelog](https://github.com/yaahc/eyre/blob/v0.6.5/CHANGELOG.md)
  - [Commits](https://github.com/yaahc/eyre/compare/v0.6.3...v0.6.5)

* Bump regex from 1.4.2 to 1.4.3. [dependabot[bot]]

  Bumps [regex](https://github.com/rust-lang/regex) from 1.4.2 to 1.4.3.
  - [Release notes](https://github.com/rust-lang/regex/releases)
  - [Changelog](https://github.com/rust-lang/regex/blob/master/CHANGELOG.md)
  - [Commits](https://github.com/rust-lang/regex/compare/1.4.2...1.4.3)



## v0.5.1 (2021-01-10)

### Fixes

* Find uppercased notes when referenced with lowercase. [Nick Groenen]

  This commit fixes a bug where, if a note contained uppercase characters
  (for example `Note.md`) but was referred to using lowercase
  (`[[note]]`), that note would not be found.



## v0.5.0 (2021-01-05)

### New

* Add --no-recursive-embeds to break infinite recursion cycles. [Nick Groenen]

  It's possible to end up with "recursive embeds" when two notes embed
  each other. This happens for example when a `Note A.md` contains
  `![[Note B]]` but `Note B.md` also contains `![[Note A]]`.

  By default, this will trigger an error and display the chain of notes
  which caused the recursion.

  Using the new `--no-recursive-embeds`, if a note is encountered for a
  second time while processing the original note, rather than embedding it
  again a link to the note is inserted instead to break the cycle.

  See also: https://github.com/zoni/obsidian-export/issues/1

* Make walk options configurable on CLI. [Nick Groenen]

  By default hidden files, patterns listed in `.export-ignore` as well as
  any files ignored by git are excluded from exports. This behavior has
  been made configurable on the CLI using the new flags `--hidden`,
  `--ignore-file` and `--no-git`.

* Support links referencing headings. [Nick Groenen]

  Previously, links referencing a heading (`[[note#heading]]`) would just
  link to the file name without including an anchor in the link target.
  Now, such references will include an appropriate `#anchor` attribute.

  Note that neither the original Markdown specification, nor the more
  recent CommonMark standard, specify how anchors should be constructed
  for a given heading.

  There are also some differences between the various Markdown rendering
  implementations.

  Obsidian-export uses the [slug] crate to generate anchors which should
  be compatible with most implementations, however your mileage may vary.

  (For example, GitHub may leave a trailing `-` on anchors when headings
  end with a smiley. The slug library, and thus obsidian-export, will
  avoid such dangling dashes).

  [slug]: https://crates.io/crates/slug

* Support embeds referencing headings. [Nick Groenen]

  Previously, partial embeds (`![[note#heading]]`) would always include
  the entire file into the source note. Now, such embeds will only include
  the contents of the referenced heading (and any subheadings).

  Links and embeds of [arbitrary blocks] remains unsupported at this time.

  [arbitrary blocks]: https://publish.obsidian.md/help/How+to/Link+to+blocks

### Changes

* Print warnings to stderr rather than stdout. [Nick Groenen]

  Warning messages emitted when encountering broken links/references will
  now be printed to stderr as opposed to stdout.

### Other

* Include filter_fn field in WalkOptions debug display. [Nick Groenen]



## v0.4.0 (2020-12-23)

### Fixes

* Correct relative links within embedded notes. [Nick Groenen]

  Links within an embedded note would point to other local resources
  relative to the filesystem location of the note being embedded.

  When a note inside a different directory would embed such a note, these
  links would point to invalid locations.

  Now these links are calculated relative to the top note, which ensures
  these links will point to the right path.

### Other

* Add brief library documentation to all public types and functions. [Nick Groenen]



## v0.3.0 (2020-12-21)

### New

* Report file tree when RecursionLimitExceeded is hit. [Nick Groenen]

  This refactors the Context to maintain a list of all the files which
  have been processed so far in a chain of embeds. This information is
  then used to print a more helpful error message to users of the CLI when
  RecursionLimitExceeded is returned.

### Changes

* Add extra whitespace around multi-line warnings. [Nick Groenen]

  This makes errors a bit easier to distinguish after a number of warnings
  has been printed.

### Other

* Setup gitchangelog. [Nick Groenen]

  This adds a changelog (CHANGES.md) which is automatically generated with
  [gitchangelog].

  [gitchangelog]: https://github.com/vaab/gitchangelog



## v0.2.0 (2020-12-13)

* Allow custom filter function to be passed with WalkOptions. [Nick Groenen]

* Re-export vault_contents and WalkOptions as pub from crate root. [Nick Groenen]

* Run mdbook hook against README.md too. [Nick Groenen]

* Update installation instructions. [Nick Groenen]

  Installation no longer requires a git repository URL now that a crate is
  published.

* Add MdBook generation script and precommit hook. [Nick Groenen]

* Add more reliable non-ASCII tetscase. [Nick Groenen]

* Create FUNDING.yml. [Nick Groenen]

## v0.1.0 (2020-11-28)

* Public release. [Nick Groenen]

<!-- 变更链接 -->

- [v26.8.2](https://github.com/ONEGAYI/obsidian-export-desktop/commits/v26.8.2)
- [v26.8.3](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.8.2...v26.8.3)
- [v26.8.4](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.8.3...v26.8.4)
- [v26.8.5](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.8.4...v26.8.5)
- [v26.8.6](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.8.5...v26.8.6)
- [v26.8.7](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.8.6...v26.8.7)
- [v26.9.0](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.8.7...v26.9.0)
- [v26.9.1](https://github.com/ONEGAYI/obsidian-export-desktop/compare/v26.9.0...v26.9.1)
