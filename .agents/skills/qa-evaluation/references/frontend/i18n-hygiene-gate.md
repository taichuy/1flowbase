# I18n Hygiene Gate

## Goal

多语言 QA 要给出可复盘证据：哪个 owner、哪个 locale、哪个 key 或 value 违反规则。不要只说“翻译不统一”。

本门禁也是 AI 复用既有文案 key 的入口：新增或转换多语言资源前，先用报告定位同 owner 内可复用 key，避免制造重复 key / value。

## Hard Boundary

用户可见文案值是开发者已调好的产品内容。除非用户在当前任务明确要求改文案，否则不得修改 locale value、按钮 / 菜单 / 标题 / 导航 / placeholder / empty / error / help text、schema label、节点展示名或默认 alias 等任何用户能看到的字符串。

i18n hygiene 的修复只能动 key、引用关系、重复 key 合并和确认失效 key 删除；必须保留原文案值。发现文案错字、语义不一致或表达问题时，只输出 finding / warning，并要求产品或开发者确认新文案。

## Command

```bash
node scripts/node/tooling.js i18n-hygiene
```

报告固定写入 `tmp/test-governance/i18n-hygiene.json`。

## Severity

- `error`: 必须修复后才能通过 QA。包括缺 locale 文件、非法 locale 文件名、非法 key 命名、中英文 key 不对齐、JSON 重复 key、同 owner 同 locale 重复 value。
- `warning`: 需要人工或 AI 复盘。默认包括前端 key 无静态代码引用；使用 `--include-cross-owner-warnings` 时额外包含跨 owner 重复 key / value advisory。只有语义完全一致且稳定时才建议上提 common，只有存在动态 key 或外部渲染入口时才保留未引用 key。

## Review Rules

- 多语言 key 的每个 JSON 段必须只使用英文小写字母；多个语义单词用 `_` 连接，例如 `primary_action`，不要用驼峰、短横线、数字、中文、空格或 `key_xxx` / `k_xxx` 这类随机生成 key。
- 同 owner 重复 value：不得修改用户前端可见文案值；优先让调用方复用已有 key，或删除确认多余的重复 key。相同文案值可保留，但 QA 报告必须说明保留原因。只有用户在当前任务明确要求改文案时，才允许调整 value。
- 跨 owner 重复 value：默认不进入门禁；专项审计时默认保留局部 owner，不要为了消灭 advisory 抽错 common。
- `unused-i18n-key`：优先删除已失效 key；如 key 由动态配置、路由配置或外部渲染入口使用，QA 报告必须写明保留原因。
- 新增 common 前先确认它是短 UI 词，不是业务句子。
- 全仓 canonical locale 固定为 `zh_Hans` 与 `en_US`；前端 UI、后端 profile / API locale、插件 / provider catalog 都使用同一套文件名和运行态 locale。
- URL、浏览器语言和 `Accept-Language` 的别名只允许在入口处归一化，不得引入前后端字段别名；接口字段名仍以 DTO / 领域语义为准。
- QA 报告必须列出命令、报告路径、error 数、warning 数和未修 warning 的保留原因；涉及 `unused-i18n-key` 时必须区分已删除、待删除和动态保留。
