# I18n Rules

## Goal

多语言资源要方便人和 AI 就近维护，也要方便脚本精确定位问题。当前只支持中文和英文。

## Locale Contract

- 全仓 canonical locale 固定为 `zh_Hans` 与 `en_US`。
- 前端 App 运行态、UI 资源文件名、用户偏好、API locale、插件 / provider catalog 都使用同一套 canonical locale。
- URL、浏览器语言和 `Accept-Language` 可以接收 `zh-CN`、`zh-Hans`、`en-US`、`en` 等别名，但进入系统后必须归一化为 `zh_Hans` 或 `en_US`。
- 不要为了展示方便改 DTO 字段名或新增 locale 别名字段。

## Placement

- 前端 UI 与插件 / provider locale 文件名统一固定为 `zh_Hans.json` 与 `en_US.json`。
- UI 文案跟随最近 owner：`app-shell/i18n`、`features/*/i18n`、`shared/ui/*/i18n`。
- 中央 i18n 入口只负责发现、注册、校验和加载，不承载全量业务文案。
- 只有跨 feature 且语义稳定的短 UI 词才能进入 common；业务句子不进 common。

## Key And Value Rules

- Key 的每个 JSON 段必须只使用英文小写字母；多个语义单词用 `_` 连接，例如 `primary_action`，不要用驼峰、短横线、数字、中文或空格。
- Key 只要求在 owner 内唯一；跨 owner 同 key 允许存在，专项复盘时可用 `i18n-hygiene --include-cross-owner-warnings` 查看 advisory warning。
- 相同展示 value 在同一 owner、同一 locale 内是 error；语义一致时让调用方复用已有 key。语义不同而暂时同文案时保留原值并报告未解决项，不为消除 error 擅自改文案。
- 跨 owner 相同 value 允许存在；只有语义完全一致且足够稳定时才上提 common，专项复盘时可用 `--include-cross-owner-warnings` 查看 advisory warning。
- 前端 `i18n/` key 没有静态代码引用是 `unused-i18n-key` warning；保留时必须说明动态 key、路由配置或外部渲染入口原因。
- 中英文文件 key 必须完全对齐；非法 key 命名、缺 key、多 key、JSON 重复 key 都是 error。
- 不要为了消灭重复字符串跨 feature 复用业务 key；错误复用比局部重复更危险。
- QA / hygiene 整理保持已有用户可见 value；修改文案需已有用户授权。API owner 提供的 catalog `summary / description` 按 DTO 消费，不借 UI 文案抽取改名或重新生成。

## QA Evidence

- 多语言资源、key 命名、文案抽取、格式转换或 common 归属变更，先核对受影响 owner 的中英文 key、原值和引用；正式 hygiene 结论运行或引用适用于当前候选的 `node scripts/node/tooling.js i18n-hygiene` 结果。
- 按 QA gate lane 选择执行面：本地局部任务复用直接相关证据，完整仓库扫描默认留给 beta / CI / 专门质量工作区；未取得报告时明确 hygiene 未验证，不把 owner 内检查写成全仓通过。
- 报告产物固定为 `tmp/test-governance/i18n-hygiene.json`。
- 交付时按脚本实际分级区分 error 与 warning；本次引入或验收范围内的 error 未解决时不能判对应验收通过，既有债务遵循 lane 的范围边界；warning 说明局部语义或动态引用原因。
- AI 新增文案前应先看同 owner 现有 key；重复 key / value 的发现辅助复用稳定语义，跨 owner advisory 不作为盲目上提 common 的理由。
