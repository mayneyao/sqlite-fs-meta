export const demoCommands = [
  "xmeta set notes/launch.md \\\n  status=Active rating:=5 --print",
  'xmeta schema set ./notes \\\n  "rating INTEGER, status TEXT"',
  'xmeta query ./notes \\\n  "SELECT id, rating, status FROM files\n   WHERE is_dir = 0 AND rating >= 4\n   ORDER BY rating DESC"',
];

export const demoFiles = ["launch.md", "design.md", "reading.md", "archive.md"];
export const demoRows = [
  { name: "launch.md", rating: 5, status: "Active" },
  { name: "reading.md", rating: 5, status: "Active" },
  { name: "design.md", rating: 4, status: "Draft" },
];

export const demoMessages = {
  en: {
    label: "See how it works",
    sample: "Product demo",
    files: "Files",
    fileCount: "4 files",
    stored: "Stored on the file",
    steps: ["Write properties", "Declare fields", "Query files"],
    captions: [
      "Attach typed properties to a file, right from the shell.",
      "Save field definitions on the folder itself, ready for the next query.",
      "Query with xmeta. SQLite and the extension are already built in.",
    ],
    saved: "Properties saved to notes/launch.md",
    created: "Saved to the folder's space.eidos.meta.schema attribute.",
    result: "3 rows",
    results: "Example SQL query results",
    pause: "Pause demo",
    play: "Play demo",
    replay: "Replay demo",
    sequence: "Demo scenes",
    command: "Example command",
    data: "Illustrative commands and results",
  },
  zh: {
    label: "看看它如何工作",
    sample: "产品演示",
    files: "文件",
    fileCount: "4 个文件",
    stored: "属性随文件保存",
    steps: ["写入属性", "声明字段", "查询文件"],
    captions: [
      "用一条命令，为文件写入有类型的属性。",
      "字段定义保存在文件夹自身，下次查询时自动读取。",
      "直接用 xmeta 查询，SQLite 和扩展已内置。",
    ],
    saved: "属性已保存到 notes/launch.md",
    created: "已保存到文件夹的 space.eidos.meta.schema 扩展属性。",
    result: "3 行结果",
    results: "SQL 查询结果示例",
    pause: "暂停演示",
    play: "播放演示",
    replay: "重新播放",
    sequence: "演示步骤",
    command: "命令示例",
    data: "命令与结果均为示意",
  },
};
