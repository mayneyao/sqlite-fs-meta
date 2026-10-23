export type Locale = "en" | "zh";

export function localePath(locale: Locale, base = "/") {
  return `${base.replace(/\/$/, "")}/${locale === "zh" ? "zh/" : ""}`;
}

const en = {
  lang: "en",
  ogLocale: "en_US",
  title: "xmeta — Custom file properties. Manage files like a database.",
  description:
    "Read, write, and query file metadata with xmeta. Folder schemas, built-in SQLite, and native xattr / ADS storage for macOS, Linux, and Windows.",
  skip: "Skip to content",
  nav: {
    home: "xmeta home",
    label: "Primary",
    demo: "Demo",
    docs: "Docs",
    switchLabel: "切换到中文",
    switchText: "中文",
  },
  copy: {
    button: "Copy",
    success: "Copied",
    successMessage: "Copied to clipboard",
    failure: "Try again",
    failureMessage: "Could not copy. Select the command and copy it manually.",
  },
  hero: {
    eyebrow: "xmeta / native file metadata CLI",
    title: ["Custom file properties.", "Manage files like a database."],
    description: [
      "Add fields, update properties, and query files with ",
      ". Metadata stays with your files. SQLite is built in.",
    ],
    install: "Install xmeta",
    docs: "Read the docs",
  },
  model: {
    label: "The model",
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
  gui: {
    label: "GUI / Eidos Lite",
    title: "A graphical interface, too.",
    description: "Eidos Lite and xmeta share the default namespace",
    detail: ". Manage the same file properties in the terminal and the GUI. File metadata management in Eidos Lite requires a Spark license.",
    action: "File metadata in Eidos Lite",
    href: "https://eidos.space/spark#file-metadata",
  },
  install: {
    platforms: "Installation platform",
    commandLabel: (platform: string) => `Install xmeta on ${platform}`,
    copyLabel: (platform: string) => `Copy ${platform} install command`,
  },
  footer: {
    truth: "Files are the source of truth.",
    label: "Footer",
    cliDocs: "CLI reference",
    extensionDocs: "SQLite extension",
    source: "Source",
    top: "Back to top",
  },
};

const zh: typeof en = {
  lang: "zh-CN",
  ogLocale: "zh_CN",
  title: "xmeta — 自定文件属性，用数据库的方式管理文件",
  description:
    "用 xmeta 读写文件属性、配置文件夹 schema、运行 SQL 查询。内置 SQLite，适用于 macOS、Linux 和 Windows。",
  skip: "跳转到正文",
  nav: {
    home: "xmeta 首页",
    label: "主导航",
    demo: "演示",
    docs: "文档",
    switchLabel: "Switch to English",
    switchText: "English",
  },
  copy: {
    button: "复制",
    success: "已复制",
    successMessage: "已复制到剪贴板",
    failure: "重试",
    failureMessage: "复制失败，请选中命令手动复制。",
  },
  hero: {
    eyebrow: "xmeta / 原生文件元数据 CLI",
    title: ["自定文件属性，", "用数据库的方式管理文件"],
    description: ["用 ", " 配置字段、读写属性、运行 SQL 查询。元数据随文件保存，SQLite 已内置。"],
    install: "安装 xmeta",
    docs: "阅读文档",
  },
  model: {
    label: "存储方式",
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
  gui: {
    label: "GUI / Eidos Lite",
    title: "用图形界面管理文件属性。",
    description: "Eidos Lite 与 xmeta 默认共用 namespace",
    detail: "，在终端和图形界面管理同一份文件属性。Eidos Lite 的文件属性管理功能需 Spark 许可证解锁。",
    action: "了解 Eidos Lite 文件属性管理",
    href: "https://eidos.space/zh/spark#file-metadata",
  },
  install: {
    platforms: "安装平台",
    commandLabel: (platform: string) => `在 ${platform} 上安装 xmeta`,
    copyLabel: (platform: string) => `复制 ${platform} 安装命令`,
  },
  footer: {
    truth: "数据，就在文件上。",
    label: "页脚导航",
    cliDocs: "CLI 参考文档",
    extensionDocs: "SQLite 扩展",
    source: "源码",
    top: "返回顶部",
  },
};

export const messages = { en, zh };
