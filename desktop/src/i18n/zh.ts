/**
 * 中文界面字典，同时是全部字典的类型基准（`Dict`）：`en.ts` 声明为
 * `Dict` 类型后，漏键/多键/结构漂移都会在编译期报错。
 *
 * 带参数的条目使用 `{name}` 占位符，经 `fmt()`（见 index.tsx）替换。
 */
export const zh = {
  common: {
    close: "关闭",
    browse: "浏览",
    // 带状态控件（开关等）的无障碍名称模板：AX 树走查工具（CUA 类）不
    // 渲染 ToggleState，状态并入名称才能在点击后被观察到；读屏会把名称
    // 与状态各播一遍，属为走查场景接受的取舍。
    statefulControl: {
      nameOn: "{title}（已开启）",
      nameOff: "{title}（已关闭）",
    },
  },
  window: {
    minimize: "最小化",
    maximize: "最大化",
    close: "关闭",
  },
  theme: {
    light: "浅色",
    dark: "深色",
    system: "跟随系统",
    toggleLabel: "主题：{current}，点击切换为{next}",
    toggleTitle: "主题：{current}（点击切换为{next}）",
  },
  language: {
    zh: "中文",
    en: "English",
    system: "跟随系统",
    menuLabel: "语言：{current}",
  },
  app: {
    warningLog: "警告",
    sidecarUnavailable: "边车不可用",
    sidecarErrorTitle: "边车进程不可用",
    // 富文本条目：三段拼出「运行 <code>…</code> 后重启应用。」中英语序均可适配。
    sidecarErrorHint: {
      pre: "运行",
      code: "just desktop-sync-sidecar",
      post: "后重启应用。",
    },
    exportTitle: "导出 Obsidian Vault",
    exportDescription:
      "将 Obsidian 方言 Markdown 转换为通用 Markdown，转换由内置的 obsidian-export 边车进程完成。",
    sourceLabel: "Vault 来源",
    sourcePlaceholder: "选择 Obsidian vault 文件夹或单篇笔记",
    destinationLabel: "导出目标",
    destinationPlaceholder: "选择输出文件夹",
    rememberPaths: "记住上次路径",
    options: "选项",
    export: "导出",
  },
  options: {
    title: "转换选项",
    description:
      "与 obsidian-export CLI 选项一一对应，改动即时保存；保持默认的选项不会传给边车。",
    sectionConversion: "转换行为",
    sectionFiltering: "内容过滤",
    sectionProcess: "文件与过程",
    sectionDiagrams: "图表渲染",
    sectionLinkCheck: "链接检查",
    sectionAbout: "关于与更新",
    updateCurrentVersion: "当前版本",
    updateIdle: "尚未检查过更新。",
    updateUnknown: "无法确定更新状态。",
    updateChecking: "正在检查更新…",
    updateCheckNowBtn: "检查更新",
    updateUpToDate: "已是最新版本。",
    updateNoRelease: "尚无任何发布版本。",
    updateAvailable: "发现新版本 {version}",
    updateNotesTitle: "更新说明",
    updateNoAsset: "当前平台没有匹配的安装包，请前往发布页手动下载。",
    updateOpenReleasePage: "打开发布页",
    updateDownload: "下载安装包",
    updateDownloading: "正在下载…",
    updateCancelDownload: "取消下载",
    updateReady: "安装包已就绪",
    updateSavedTo: "已保存至 {path}",
    updateInstall: "安装更新",
    updateInstallHint: "启动安装向导，应用将自动退出；安装完成后重新打开即可。",
    updateFailed: "检查或下载未能完成。",
    updateAutoCheckTitle: "自动检查更新",
    updateAutoCheckHint: "启动时检查新版本（每天至多一次）",
    frontmatterLabel: "Frontmatter 处理",
    missingSectionLabel: "缺失章节的处理方式",
    commentsLabel: "Obsidian 注释（%% 围栏）的处理",
    hardLinebreaks: {
      title: "硬换行",
      description: "软换行转为硬换行，贴近 Obsidian「严格换行」设置",
    },
    noRecursiveEmbeds: {
      title: "非递归嵌入",
      description: "不展开嵌入中的嵌套嵌入，可打断笔记间的循环引用",
    },
    hidden: {
      title: "包含隐藏文件",
      description: "导出以 . 开头的隐藏文件（默认跳过）",
    },
    noGit: {
      title: "禁用 git 集成",
      description: "不读取 .gitignore 忽略规则（默认读取）",
    },
    ignoreFileLabel: "忽略规则文件名",
    ignoreFilePlaceholder: ".export-ignore（默认）",
    skipTagsLabel: "跳过标签",
    skipTagsPlaceholder: "含任一标签的笔记不导出",
    onlyTagsLabel: "仅导出标签",
    onlyTagsPlaceholder: "只导出含任一标签的笔记",
    startAtLabel: "仅导出子路径（可选）",
    startAtPlaceholder: "选择 vault 内的子文件夹，留空导出全部",
    startAtHint: "需位于 vault 根目录之下，越界会在导出时报错",
    preserveMtime: {
      title: "保留修改时间",
      description: "导出文件保持与源笔记相同的修改时间",
    },
    failFast: {
      title: "快速失败",
      description: "遇到第一个失败文件立即停止，而非继续并在末尾汇总",
    },
    linkCheckEnable: {
      title: "导出后链接检查",
      description: "导出成功后自动运行链接检查，逐条报告失效链接",
    },
    linkCheckTargetLabel: "检查目标",
    // 检查目标选项：与 OptionsView 单选列表共用文案。
    linkCheckTargetChoices: {
      source: {
        label: "Vault 源",
        description: "检查 vault 原文，可发现转换前的死链（wikilink、嵌入、标准链接）",
      },
      destination: {
        label: "导出结果",
        description:
          "检查导出产物，验证生成的 Markdown 链接与锚点；死链 wikilink 在导出时已塌缩为纯文本，不再可查",
      },
    },
    footer: "全部选项均已记住，下次启动自动沿用。",
    resetDefaults: "恢复默认",
    back: "返回",
    diagramsDescription:
      "导出时把特殊代码块（dot、Mermaid 等）调用本机工具渲染为图片，并以标准图片引用嵌入产物。工具从 PATH 自动查找；缺失时导出开始前即报错退出，不会写出任何文件。",
    diagramRenderersLabel: "启用的渲染器",
    // 渲染器选项：药丸复选的标签与悬浮说明。
    diagramRendererChoices: {
      dot: {
        label: "dot",
        description: "Graphviz DOT 图（dot / graphviz 代码块），需要 dot",
      },
      mermaid: {
        label: "Mermaid",
        description: "Mermaid 图表（mermaid 代码块），需要 mmdc（mermaid-cli）",
      },
      wavedrom: {
        label: "WaveDrom",
        description: "数字时序图（wavedrom 代码块），需要 wavedrom",
      },
      tikz: {
        label: "TikZ",
        description: "TikZ 绘图（tikz 代码块），需要 latex 与 dvisvgm（TeX 发行版）；图内中文可能渲染异常",
      },
    },
    diagramFormatLabel: "输出格式",
    diagramFormatChoices: {
      svg: {
        label: "SVG",
        description: "矢量格式，任意缩放不失真（默认）",
      },
      png: {
        label: "PNG",
        description: "位图格式，兼容性最好",
      },
    },
    diagramFormatFallbackNote:
      "所选格式渲染器不支持时自动回落为 SVG 并提示（WaveDrom、TikZ 无 PNG 输出）。",
    diagramBinsTitle: "高级：可执行文件路径",
    diagramBinsHint:
      "默认从 PATH 自动查找。仅当工具不在 PATH 或需要指定版本时填写；路径无效会在导出前报错。",
    diagramBinsPlaceholder: "留空则从 PATH 查找",
    diagramToolNames: {
      dot: "dot（Graphviz）",
      mmdc: "mmdc（mermaid-cli）",
      wavedrom: "wavedrom",
      latex: "latex（TeX）",
      dvisvgm: "dvisvgm（TeX）",
    },
    // 选项值的双语文案：OptionsView 单选列表与导出确认摘要共用。
    frontmatterChoices: {
      auto: {
        label: "自动",
        description: "笔记自带 frontmatter 时原样保留（默认）",
      },
      always: {
        label: "始终添加",
        description: "没有 frontmatter 的笔记也补一个空的 frontmatter 块",
      },
      never: {
        label: "全部移除",
        description: "导出结果不包含任何 frontmatter",
      },
    },
    missingSectionChoices: {
      skip: {
        label: "跳过",
        description: "嵌入置空并发警告（默认，贴近 Obsidian 行为）",
      },
      "embed-full": {
        label: "嵌入整篇",
        description: "找不到章节时嵌入整篇笔记（旧行为）",
      },
      fail: {
        label: "报错",
        description: "该笔记导出失败并计入结果",
      },
    },
    commentsChoices: {
      keep: {
        label: "保留原样",
        description: "%% 注释按字面保留（默认）",
      },
      convert: {
        label: "转为 HTML 注释",
        description: "转换为 <!-- -->，源码可见但渲染时隐藏",
      },
      strip: {
        label: "彻底移除",
        description: "从导出结果中删除注释",
      },
    },
    // summarizeOptions 的条目文案（导出确认弹窗中的摘要行）。
    summary: {
      startAt: "仅导出 {name}",
      frontmatter: "Frontmatter：{label}",
      ignoreFile: "忽略文件：{name}",
      skipTags: "跳过标签 ×{n}",
      onlyTags: "仅导出标签 ×{n}",
      hidden: "含隐藏文件",
      noGit: "禁用 git",
      noRecursiveEmbeds: "非递归嵌入",
      preserveMtime: "保留修改时间",
      missingSection: "缺失章节：{label}",
      failFast: "快速失败",
      hardLinebreaks: "硬换行",
      comments: "注释：{label}",
      linkCheck: "导出后链接检查（{target}）",
      diagramRenderers: "图表渲染 ×{n}（{format}）",
      diagramBins: "自定义工具路径 ×{n}",
    },
  },
  dialog: {
    title: "导出确认",
    activeOptions: "生效选项",
    modify: "修改",
    allDefault: "全部保持默认",
    keepRootTitle: "在目标下保留根文件夹",
    keepRootDescription:
      "导出文件夹时写入「目标/{name}」，避免内部第一层文件散落在目标位置（仅文件夹来源生效）。",
    keepRootFallbackName: "来源文件夹名",
    cancel: "取消",
    start: "开始导出",
  },
  run: {
    title: "正在导出…",
    progressCount: "{processed} / {total} 篇笔记",
    doneCount: "{n} 成功",
    skippedCount: "{n} 跳过",
    failedCount: "{n} 失败",
    diagramProgress: "正在渲染图表 {index}/{total}（{language}）",
    waiting: "等待边车事件…",
    cancel: "取消导出",
  },
  result: {
    cancelled: "导出已取消",
    aborted: "导出异常终止",
    partial: "导出完成（部分失败）",
    completed: "导出完成",
    abortedDetail: "事件流未正常终结，以下为已处理的部分。",
    summary: "共 {total} 篇：{done} 成功 · {skipped} 跳过 · {failures} 失败",
    warnings: "{n} 警告",
    back: "返回",
  },
  // 导出后自动链接检查的报告面板：状态文案将结构化判定本地化，
  // 文件路径与链接原文保持技术原文透传。
  linkCheck: {
    runningTitle: "正在检查链接…",
    runningProgress: "已报告 {n} 条链接",
    titleClean: "链接检查：全部通过",
    titleBroken: "链接检查：{n} 处失效",
    titleFailed: "链接检查未能完成",
    failedHint: "事件流未正常终结。",
    exitCode: "退出码 {code}",
    cancel: "取消检查",
    summary: "{files} 个文件 · {links} 条链接 · {broken} 失效 · {skipped} 跳过（外部链接）",
    filter: {
      broken: "仅失效",
      all: "全部",
      skipped: "跳过",
    },
    truncated: "仅显示前 {shown} 条，共 {total} 条",
    emptyList: "该筛选下没有条目",
    statusOk: "正常",
    statusMissingFile: "目标不存在：{target}",
    statusOutOfBounds: "越出检查根：{target}",
    statusMissingSection: "{target} 中不存在章节「{section}」",
    statusMissingBlock: "{target} 中不存在块 ^{block}",
    statusUnreadable: "文件不可读：{message}",
    statusExternal: "外部链接，跳过：{url}",
    statusUnknown: "未知状态",
    kinds: {
      wikiLink: "Wikilink",
      wikiEmbed: "嵌入",
      markdownLink: "Markdown 链接",
      markdownImage: "Markdown 图片",
      unknown: "未知类型",
    },
  },
  tagInput: {
    removeTag: "移除标签 {tag}",
    placeholder: "输入后回车添加",
  },
} as const;

/**
 * The zh dictionary's shape with leaf literals widened to `string`, so the
 * English dictionary can be typed as `Dict`: keys must match exactly (extra,
 * missing, or structurally drifted keys fail the build) while values are free
 * translations.
 */
type Widen<T> = T extends string ? string : { [K in keyof T]: Widen<T[K]> };

export type Dict = Widen<typeof zh>;
