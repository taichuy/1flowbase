# Agent Logs 源码采集 CLI

仓库提供可直接执行的 **源码 CLI**，当前不声明已有 npm 发布包。使用 Node.js 20 或更高版本，保留 `scripts/node/agent-logs-collector.js` 与同名目录；无需安装额外依赖。源码交付和未来发布包是两件事，不要使用未经发布的 `npx` 包名。

创建 Agent Logs 应用后，通过 shell 或密钥管理器设置 `FLOWBASE_AGENT_LOGS_API_KEY`。密钥不作为命令参数，不输出到日志。来源路径由用户明确选择，CLI 不会自动扫描用户主目录。

```bash
node scripts/node/agent-logs-collector.js import \
  --endpoint https://your-host/api/logs/v1/events \
  --source /your/chosen/codex-directory \
  --state /your/private/agent-logs-state.json

node scripts/node/agent-logs-collector.js watch \
  --endpoint https://your-host/api/logs/v1/events \
  --source /your/chosen/codex-directory \
  --state /your/private/agent-logs-state.json
```

选择包含 `sessions` 和 `archived_sessions` 的 Codex 根目录即可导入历史及归档数据，也可指定单个 `.jsonl` 文件。递归扫描全部选中 JSONL，不设文件数或事件总量上限；跳过符号链接以防离开指定目录或产生递归环。`--batch-size` 只控制传输批次大小（默认 100），`--interval-ms` 控制 watch 轮询间隔（默认 2000）。watch 失败后下轮重试，import 出现上传、解析或断点错误会失败退出。全部选项见 `--help`。

`--endpoint` 必须是完整 ingest 地址。HTTP 可用于本地开发，跨网络传输应用密钥使用 HTTPS，CLI 拒绝重定向。断点保存来源身份、目标地址、客户端类型和已确认偏移，不保存密钥或对话正文。来源和断点属于本地私有数据。恢复时保留断点及生成的 `source_id`；`--source-id` 可显式指定稳定安装身份，但必须与已有断点一致。更换 endpoint 或 adapter 使用独立断点。同一断点只允许一个采集进程；异常退出后，确认没有活跃采集进程再删除相邻 `.lock` 文件。

## 恢复与来源语义

只读取以换行符结束的完整 JSONL 行，末尾半行即使暂时可解析也不提交，补全后继续。只有完整持久 ACK 才推进断点；网络失败、请求拒绝或 ACK 不完整保留旧断点。服务端提交后、断点落盘前崩溃会重放，服务端按稳定事件身份判重。已确认前缀被改写或截断时拒绝继续。每轮重新解析来源上下文，再从已确认位置恢复。

事件身份来自不可变的首行 rollout header 和字节位置，移入 `archived_sessions` 不会改变。不同位置的相同文本保留为不同事实，不凭内容猜测去重。事件时间取来源时间，不取采集主机时间。optional ordinal、`history_base` 前缀引用、parent thread、root session 均保留在 `raw`，不会与本地字节偏移或子 thread 身份混淆。

只有来源明确提供的 turn ID 才建立任务：来自 `turn_context`、task/turn-start、typed usage 或 response-item passthrough metadata。初始 session 和轮次前事实等待首个可靠 turn，随后归属该轮次，事件身份不变。没有可靠 turn 的文件会报告等待来源轮次身份，不捏造用户任务，也不推进断点。前缀引用保留为事实，不复制父 rollout 来合成子任务事件。来源已内联的继承历史按 metadata/ordinal 边界标记，后端从新任务输入和 usage 中排除继承事实。

Codex `response_item` user/assistant 提供对话事实；只有明确 `phase: "final_answer"` 才声明终答。commentary、工具、compaction、上下文、未知类型及原始事实保留轨迹。`event_msg` user/agent 展示镜像保存为 context，避免重复形成对话记录。结束事件只记录 task_end，不从 `last_agent_message` 制造终答。旧版本没有明确 phase 时保留未知，model/provider/phase 不猜测；只复制明确提供的 `model` 和 `model_provider`，不更名 provider 标识。

新 `token_usage_record.usage` 是带 `response_id` 的 response delta，`turn_token_usage/thread_token_usage` 的累计值保留 raw。旧 `event_msg.token_count.info.total_token_usage` 使用 `basis: "cumulative"`，`last_token_usage` 保留 raw。累计快照不能与 response delta 相加。聚合、价格和计费由后端负责，CLI 不计算费用。

## API 与 adapter

向 `POST /api/logs/v1/events` 发送 `Authorization: Bearer <application-key>`。固定 `schema_version: "1flowbase.agent-logs/v1"`；精确 JSON 字段与示例见 [English API contract](README.md#api-envelope-and-receipt)。kind 为 `system/user/assistant/tool_call/tool_result/usage/context/task_end`。usage 的 basis 为 `delta/cumulative`，可含 `response_id/input_tokens/output_tokens/input_cache_hit_tokens/cache_write_tokens/total_tokens`。

成功 receipt 含 `accepted_events`、`duplicate_events`、字符串数组 `record_ids`；接受数与重复数之和必须等于完整批次事件数。服务端整批持久提交或整批失败；相同身份不同 payload 为冲突。应用 key 授权日志写入，不授权模型生成。

`--adapter /absolute/path/to/installed-adapter.cjs` 可选择已安装的可信 CommonJS 脚本。adapter 以采集进程权限执行本地代码，仅选择可信脚本。单一 loader 要求导出 `sourceClient`、`createContext(firstLine)` 和 `convert(line, context, position)`；convert 返回不含 event_id/sequence 的规范事件或 null，`position.start/end` 为字节偏移，source_task_id=null 表示等待可靠归属。共享调度负责身份、上传和断点，adapter 只转换格式、维护确定性解析上下文，不自行实现 HTTP 或 checkpoint。这是本地来源 adapter 边界，不是服务端 runtime 插槽。

内置 adapter 对照用户提供的本地 Codex Rust protocol/history 定义；来源版本漂移和缺少明确元数据仍是适用边界。集中 QA 执行 fixtures：

```bash
node --test scripts/node/agent-logs-collector/_tests/collector.test.js
```
