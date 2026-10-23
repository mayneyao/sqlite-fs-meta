export const demoScenes = [
  {
    command: "ls",
    output: "Arrival.mkv  Dune.mkv  Her.mkv  Severance.S01E01.mkv",
  },
  {
    command: 'xmeta schema set . "genre TEXT, status TEXT, rating REAL" --json',
    output: '{"fields":[{"name":"genre","type":"TEXT"},{"name":"status","type":"TEXT"},{"name":"rating","type":"REAL"}],"version":1}',
  },
  {
    command: "xmeta set Her.mkv genre=Sci-Fi status=Watched rating:=8.5 --print",
    output: '{\n  "genre": "Sci-Fi",\n  "rating": 8.5,\n  "status": "Watched"\n}',
  },
  {
    command: `xmeta query . "SELECT id, status, rating FROM files WHERE is_dir = 0 AND genre = 'Sci-Fi' AND rating >= 8 ORDER BY rating DESC"`,
    output: "id\tstatus\trating\nHer.mkv\tWatched\t8.5\n1 row(s)",
  },
];

export const demoMessages = {
  en: {
    label: "A film and TV library",
    sample: "Product demo",
    steps: ["List files", "Declare fields", "Write properties", "Query files"],
    captions: [
      "Start with the movies and shows already in your folder.",
      "Define genre, watch status, and your own rating.",
      "Mark Her as watched and add a personal rating.",
      "Find science-fiction picks you rated 8 or higher.",
    ],
    transcript: "Example film library terminal session",
    command: "Example command",
    output: "Command output",
    pause: "Pause demo",
    play: "Play demo",
    replay: "Replay from ls",
    data: "Illustrative commands and results",
  },
  zh: {
    label: "影视文件管理",
    sample: "产品演示",
    steps: ["查看文件", "声明字段", "写入属性", "查询文件"],
    captions: [
      "从已有的电影和剧集文件开始。",
      "自定类型、观看状态和评分字段。",
      "为《她》记录类型、已看状态和个人评分。",
      "用 SQL 筛出个人评分不低于 8 分的科幻作品。",
    ],
    transcript: "影视管理命令行示例",
    command: "命令示例",
    output: "命令输出",
    pause: "暂停演示",
    play: "播放演示",
    replay: "从 ls 重新播放",
    data: "命令与结果均为示意",
  },
};
