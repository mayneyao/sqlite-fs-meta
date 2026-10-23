export const demoScenes = [
  {
    command: "ls notes",
    output: "draft.md  ideas.md  plan.md  readme.md",
  },
  {
    command: "xmeta set notes/plan.md status=Active rating:=5 --print",
    output: '{\n  "rating": 5,\n  "status": "Active"\n}',
  },
  {
    command: 'xmeta schema set notes "rating INTEGER, status TEXT" --json',
    output: '{"fields":[{"name":"rating","type":"INTEGER"},{"name":"status","type":"TEXT"}],"version":1}',
  },
  {
    command: 'xmeta query notes "SELECT id, rating, status FROM files WHERE is_dir = 0 AND rating >= 4 ORDER BY rating DESC"',
    output: "id\trating\tstatus\nplan.md\t5\tActive\n1 row(s)",
  },
];

export const demoMessages = {
  en: {
    label: "See how it works",
    sample: "Product demo",
    steps: ["List files", "Write properties", "Declare fields", "Query files"],
    captions: [
      "Start with the files you already have.",
      "Add your own typed properties, right from the shell.",
      "Save field definitions on the folder itself.",
      "Filter and sort your files with SQL. SQLite is built in.",
    ],
    transcript: "Example terminal session",
    command: "Example command",
    output: "Command output",
    pause: "Pause demo",
    play: "Play demo",
    replay: "Replay from ls",
    sequence: "Demo steps",
    data: "Illustrative commands and results",
  },
  zh: {
    label: "看看它如何工作",
    sample: "产品演示",
    steps: ["查看文件", "写入属性", "声明字段", "查询文件"],
    captions: [
      "从已有文件开始。",
      "在命令行中，为文件添加自定义属性。",
      "字段定义保存在文件夹自身。",
      "用 SQL 筛选、排序文件。SQLite 已内置。",
    ],
    transcript: "命令行会话示例",
    command: "命令示例",
    output: "命令输出",
    pause: "暂停演示",
    play: "播放演示",
    replay: "从 ls 重新播放",
    sequence: "演示步骤",
    data: "命令与结果均为示意",
  },
};
