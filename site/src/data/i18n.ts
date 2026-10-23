export type Locale = "en" | "zh";

export function localePath(locale: Locale, base = "/") {
  return `${base.replace(/\/$/, "")}/${locale === "zh" ? "zh/" : ""}`;
}

const en = {
  lang: "en",
  ogLocale: "en_US",
  title: "xmeta — File metadata, from the terminal.",
  description:
    "Read, write, and query file metadata with xmeta. Folder schemas, built-in SQLite, and native xattr / ADS storage for macOS, Linux, and Windows.",
  skip: "Skip to content",
  nav: {
    home: "xmeta home",
    label: "Primary",
    how: "How to use",
    docs: "Docs",
    switchLabel: "切换到中文",
    switchText: "中文",
  },
  copy: {
    button: "Copy",
    commands: "Copy commands",
    success: "Copied",
    successMessage: "Copied to clipboard",
    failure: "Try again",
    failureMessage: "Could not copy. Select the command and copy it manually.",
    example: "Example",
    codeLabel: (label: string) => `${label} code`,
    label: (label: string) => `Copy ${label}`,
    install: "Copy xmeta install command",
    unix: "Copy macOS and Linux install command",
    windows: "Copy Windows install command",
    source: "Copy source install commands",
  },
  hero: {
    eyebrow: "xmeta / native file metadata CLI",
    title: ["File metadata,", "from the terminal."],
    description: [
      "Read, write, and query file metadata with ",
      ". SQLite is built in. Your files stay the source of truth.",
    ],
    install: "Install xmeta",
    docs: "Read the docs",
  },
  model: {
    label: "01 / The model",
    title: ["The metadata", "stays with the file."],
    description:
      "Properties live in native extended attributes or NTFS alternate data streams. Your folder stays the source of truth.",
    details: [
      {
        title: "The folder remembers its fields.",
        text: "Store field definitions in the folder's extended attributes. xmeta reads them automatically when you query.",
      },
      {
        title: "Read where it lives.",
        text: "Metadata is read at query time. A lookup by relative path resolves the file directly, without walking the entire folder.",
      },
      {
        title: "Write it once.",
        text: "The CLI and SQL extension read and write the same JSON envelope. No separate index to maintain.",
      },
    ],
  },
  interfaces: {
    label: "02 / The interfaces",
    title: ["Start with xmeta.", "Integrate with SQLite."],
    description: [
      "Use ",
      " directly in your terminal or agent workflow. Embed the same extension in an existing SQLite application when you need integration.",
    ],
    extension: "SQLite extension",
    query: "For integrations",
    extensionDocs: "Extension reference",
    cli: "xmeta CLI",
    edit: "For people and agents",
    shell: "Shell",
    cliDocs: "CLI reference",
    boundaries: "A few boundaries",
    note: "Writes require xattr / ADS support. Symlinks, Windows reparse points, and Unix files with multiple hard links are rejected for writes. Directory properties are not inherited.",
  },
  agent: {
    action: "Use with an agent",
    eyebrow: "03 / For agents",
    title: ["Let your agent", "handle the metadata."],
    description:
      "Copy a starter prompt into your agent. It can read the documentation, install xmeta if needed, and help with your file metadata task.",
    promptLabel: "Agent starter prompt",
    copy: "Copy prompt",
    copyLabel: "Copy agent starter prompt",
    preview: "Read the prompt",
    docsLabel: "Agent documentation",
    guide: "Agent guide",
    full: "Full reference",
  },
  install: {
    platforms: "Installation platform",
    commandLabel: (platform: string) => `Install xmeta on ${platform}`,
    copyLabel: (platform: string) => `Copy ${platform} install command`,
    unixNote: "Installs to ~/.local/bin. Re-run to update; add the directory to PATH if needed.",
    windowsNote: "Run in PowerShell. Installs the x86-64 CLI and adds it to your user PATH.",
  },
  footer: {
    truth: "Files are the source of truth.",
    label: "Footer",
    cliDocs: "CLI reference",
    source: "Source",
    top: "Back to top",
  },
};

const zh: typeof en = {
  lang: "zh-CN",
  ogLocale: "zh_CN",
  title: "xmeta — 在命令行读写与查询文件元数据。",
  description:
    "用 xmeta 读写文件属性、配置文件夹 schema、运行 SQL 查询。内置 SQLite，适用于 macOS、Linux 和 Windows。",
  skip: "跳转到正文",
  nav: {
    home: "xmeta 首页",
    label: "主导航",
    how: "使用方法",
    docs: "文档",
    switchLabel: "Switch to English",
    switchText: "English",
  },
  copy: {
    button: "复制",
    commands: "复制命令",
    success: "已复制",
    successMessage: "已复制到剪贴板",
    failure: "重试",
    failureMessage: "复制失败，请选中命令手动复制。",
    example: "示例",
    codeLabel: (label: string) => `${label}代码`,
    label: (label: string) => `复制${label}`,
    install: "复制 xmeta 安装命令",
    unix: "复制 macOS 和 Linux 安装命令",
    windows: "复制 Windows 安装命令",
    source: "复制源码安装命令",
  },
  hero: {
    eyebrow: "xmeta / 原生文件元数据 CLI",
    title: ["文件元数据，", "从命令行开始。"],
    description: ["用 ", " 读写文件属性、配置字段、运行 SQL 查询。SQLite 已内置，安装即可使用。"],
    install: "安装 xmeta",
    docs: "阅读文档",
  },
  model: {
    label: "01 / 存储方式",
    title: ["元数据，", "与文件放在一起。"],
    description: "属性保存在原生扩展属性或 NTFS 备用数据流中。文件夹本身就是数据来源。",
    details: [
      {
        title: "文件夹，记住自己的字段。",
        text: "字段定义保存在文件夹的扩展属性中，xmeta 查询时自动读取。",
      },
      {
        title: "查询时，直接读取。",
        text: "元数据在查询时读取。按相对路径查找可直接定位文件，无需遍历整个文件夹。",
      },
      {
        title: "只维护一份元数据。",
        text: "CLI 和 SQL 扩展读写同一份 JSON 数据，无需维护额外索引。",
      },
    ],
  },
  interfaces: {
    label: "02 / 使用方式",
    title: ["直接用 xmeta，", "集成用 SQLite。"],
    description: [
      "在终端或 Agent 工作流中直接使用 ",
      "。需要对接已有应用时，再将同一扩展嵌入 SQLite。",
    ],
    extension: "SQLite 扩展",
    query: "面向应用集成",
    extensionDocs: "扩展参考文档",
    cli: "xmeta CLI",
    edit: "面向用户与 Agent",
    shell: "命令行",
    cliDocs: "CLI 参考文档",
    boundaries: "使用限制",
    note: "写入需要文件系统支持 xattr / ADS。不支持向符号链接、Windows 重解析点或具有多个硬链接的 Unix 文件写入。文件不会继承目录的属性。",
  },
  agent: {
    action: "交给 Agent",
    eyebrow: "03 / 面向 Agent",
    title: ["让 Agent", "管理文件属性。"],
    description:
      "把提示词复制给你的 Agent，让它阅读文档、按需安装 xmeta，并协助完成文件元数据任务。",
    promptLabel: "Agent 起始提示词",
    copy: "复制提示词",
    copyLabel: "复制 Agent 起始提示词",
    preview: "查看提示词",
    docsLabel: "Agent 文档",
    guide: "Agent 指南",
    full: "完整参考",
  },
  install: {
    platforms: "安装平台",
    commandLabel: (platform: string) => `在 ${platform} 上安装 xmeta`,
    copyLabel: (platform: string) => `复制 ${platform} 安装命令`,
    unixNote: "安装到 ~/.local/bin，重复运行即可更新；如有需要，将该目录加入 PATH。",
    windowsNote: "在 PowerShell 中运行。安装 x86-64 CLI，并加入用户 PATH。",
  },
  footer: {
    truth: "数据，就在文件上。",
    label: "页脚导航",
    cliDocs: "CLI 参考文档",
    source: "源码",
    top: "返回顶部",
  },
};

export const messages = { en, zh };
