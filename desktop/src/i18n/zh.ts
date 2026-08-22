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
    frontmatterLabel: "Frontmatter 处理",
    missingSectionLabel: "缺失章节的处理方式",
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
    footer: "全部选项均已记住，下次启动自动沿用。",
    resetDefaults: "恢复默认",
    back: "返回",
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
