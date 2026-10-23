import type { Locale } from "./i18n";
import { installCommand, siteOrigin, windowsInstallCommand } from "./project";

export const agentPrompts: Record<Locale, string> = {
  en: `Help me manage metadata on my local files with xmeta.

First, ask which existing files or folder I want to work with, which properties and values to use, and whether I want to read or change them. Ask only for details I have not already provided.

Read ${siteOrigin}/llms.txt and follow the linked agent guide and CLI reference. Then:

1. Check xmeta --version and xmeta --help. If xmeta is missing, install the standalone CLI for my platform using the official installer:
   macOS / Linux: ${installCommand}
   Windows PowerShell: ${windowsInstallCommand}
   If the environment cannot run commands or install software, show me the next command and explain the limit.
2. Inspect configuration with xmeta --json config get from the target directory (or a file's parent directory). Read existing properties with xmeta --json get "<path>" before writing. Respect each target's namespace; if I specified one, pass it explicitly with --namespace. Do not change configuration unless the task requires it.
3. Carry out the changes I requested, limited to those files and properties. KEY=VALUE stores a string; KEY:=JSON stores typed JSON. Prefer merge updates and --dry-run. Preserve unrelated properties and file contents. Use unset only for requested keys; clear and --replace remove other properties, so use them only when the task explicitly calls for that.
4. For folder queries, use the built-in xmeta query "<folder>" "<SELECT SQL>" --json. SQLite and fs_meta are bundled; no separate installation or .load step is required. Inspect xmeta schema get "<folder>" first. If the task calls for new fields, use xmeta schema set on that root folder; it stores field definitions in <namespace>.schema without rewriting file properties. Use --params with a JSON array for SQL values and --limit / --offset for pages. Queries are read-only; use the existing metadata commands to edit files. The separate SQLite extension is for application integration. Do not invent scan, serve, or web UI commands. Use Eidos Lite as the general-purpose GUI; its file metadata management feature requires a Spark license.
5. Re-read the metadata after changes and report the paths, namespace, before/after values, xmeta version, and any errors. CLI writes take effect immediately; SQL rollback does not undo them. Stop on unsupported filesystems or invalid metadata instead of silently replacing data.`,
  zh: `请用 xmeta 帮我管理本地文件的元数据。

先了解我要操作哪些已有文件或文件夹、涉及哪些属性和值，以及是读取还是修改。只询问我尚未提供的信息。

阅读 ${siteOrigin}/llms.txt，并按其中链接的 Agent 指南和 CLI 参考文档操作：

1. 检查 xmeta --version 和 xmeta --help。若尚未安装，使用当前系统的官方脚本安装独立 CLI：
   macOS / Linux：${installCommand}
   Windows PowerShell：${windowsInstallCommand}
   如果当前环境不能执行命令或安装软件，说明限制并给出下一条命令。
2. 在目标目录（或文件所在的父目录）中用 xmeta --json config get 检查配置，写入前先用 xmeta --json get "<路径>" 读取属性。尊重各目标的命名空间；若我指定了命名空间，显式传入 --namespace。除非任务需要，否则不改配置。
3. 仅对指定文件和属性执行我要求的修改。KEY=VALUE 写入字符串，KEY:=JSON 写入 JSON 类型。优先合并更新，并用 --dry-run 预览。保留其他属性和文件内容；仅对要求删除的键使用 unset。clear 和 --replace 会移除其他属性，仅在任务明确要求时使用。
4. 文件夹查询直接使用 xmeta query "<文件夹>" "<SELECT SQL>" --json。SQLite 与 fs_meta 已内置，不需要单独安装或 .load。先用 xmeta schema get "<文件夹>" 检查字段；任务需要新增字段时，在该根目录使用 xmeta schema set，将定义存到 <命名空间>.schema，不会重写文件属性。SQL 值用 --params 的 JSON 数组绑定，结果用 --limit / --offset 分页。query 只读；修改文件继续使用已有元数据命令。单独的 SQLite 扩展用于应用集成。不要编造 scan、serve 或 Web UI 命令，通用图形界面使用 Eidos Lite，其文件属性管理功能需 Spark 许可证解锁。
5. 修改后重新读取，汇报具体路径、命名空间、修改前后的值、xmeta 版本和错误。CLI 写入即时生效，不能通过 SQL 回滚；遇到不支持的文件系统或无效元数据时停止并说明，不要静默覆盖。`,
};
