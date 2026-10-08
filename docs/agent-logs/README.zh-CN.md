# Agent Logs 原生采集器

[English](README.md)

采集器分两步安装：先从远程扩展目录**安装到 1flowbase**，再打开“下载采集 CLI”详情，在运行 Codex 的本机执行命令。安装器、原生包、校验文件和文档均从当前 1flowbase 的版本固定 URL 下载；远程仓库不可用时，已安装版本仍可下载。“平台已安装”表示分发包已留存在平台，不表示用户本机已安装或在线。

在 Agent Logs 应用的 **采集 CLI** 页面点击 Codex 页签进入详情；“全部”页签回到目录。可以粘贴当前应用的 API Key，或点击“快速生成 Key”创建并回填一个真实应用 Key。复制 Shell 或 PowerShell 安装命令，在运行 Codex 的用户电脑执行；未填写 Key 时，安装器仍会在本地终端提示输入。官方 Rust 可执行程序不要求安装 Node.js、Rust 工具链或检出源码。

安装器下载并校验原生发行，保存私有配置，启动用户级后台服务。Linux 使用用户 systemd，macOS 使用 LaunchAgent，Windows 使用用户计划任务；同一用户重启登录后恢复。没有可用后台机制时明确报告；`--no-start` 可配置后交给自己的进程管理器运行。

每个应用 ID 对应独立安装和断点。默认来源是安装时的 `CODEX_HOME`，未设置时使用 `~/.codex`，只读取 `sessions` 和 `archived_sessions` 中的已有历史及后续完整记录；支持自定义来源路径。不修改 Codex 配置或源日志。

输入框只保留当前详情页内存，刷新或离开详情后清空，不写入浏览器存储或查询缓存。命令预览遮住 Key，复制命令使用真实值，通过 `FLOWBASE_AGENT_LOGS_API_KEY` 环境变量交给本地安装器；Key 不出现在下载 URL 或安装器 CLI 参数中。安装器将 Key 保存到本机私有 `config.json`。快速生成的 Key 可在应用 API 页面管理或撤销。上传仅通过 Bearer header 发送给指定 1flowbase 端点。端点是完整 `/api/logs/v1/events` URL；网络传输应使用 HTTPS，上传拒绝重定向。

[官方安装、升级与卸载说明](https://github.com/taichuy/1flowbase-official-plugins/blob/main/runtime-extensions/@taichuy/codex-logs-collector/README.md)

原生命令为 `codex-logs-collector import --config PATH` 和 `codex-logs-collector watch --config PATH`。升级保留 `state.json` 及其自动生成的来源身份。一个断点只能由一个进程持有；异常退出由操作系统释放锁，无需删除断点。重新配置可更换 Key，改变 endpoint/source 使用独立安装。

## 恢复与来源语义

只读取以换行符结束的完整 JSONL 行，末尾半行即使暂时可解析也不提交，补全后继续。只有完整持久 ACK 才推进断点；网络失败、请求拒绝或 ACK 不完整保留旧断点。服务端提交后、断点落盘前崩溃会重放，服务端按稳定事件身份判重。已确认前缀被改写或截断时拒绝继续。每轮重新解析来源上下文，再从已确认位置恢复。

事件身份来自不可变的首行 rollout header 和字节位置，移入 `archived_sessions` 不会改变。不同位置的相同文本保留为不同事实，不凭内容猜测去重。事件时间取来源时间，不取采集主机时间。optional ordinal、`history_base` 前缀引用、parent thread、root session 均保留在 `raw`，不会与本地字节偏移或子 thread 身份混淆。

只有来源明确提供的 turn ID 才建立任务：来自 `turn_context`、task/turn-start、typed usage 或 response-item passthrough metadata。初始 session 和轮次前事实等待首个可靠 turn，随后归属该轮次，事件身份不变。没有可靠 turn 的文件会报告等待来源轮次身份，不捏造用户任务，也不推进断点。前缀引用保留为事实，不复制父 rollout 来合成子任务事件。来源已内联的继承历史按 metadata/ordinal 边界标记，后端从新任务输入和 usage 中排除继承事实。

Codex `response_item` user/assistant 提供对话事实；明确标记 `phase: "final_answer"` 的 assistant 消息声明终答。commentary、工具、compaction、上下文、未知类型及原始事实保留轨迹。`event_msg` user/agent 展示镜像保存为 context，避免重复形成对话记录。Codex `task_complete`（wire alias 为 `turn_complete`）通过非空字符串 `last_agent_message` 明确声明完成文本；adapter 将该事实映射为 `kind: "task_end", phase: "final_answer", content: <来源文本>`，保留真实 turn 归属和完整 raw。完成文本缺失、为空或仅含空白时，仍保留 `content: null, phase: null`；不生成摘要，也不从普通 assistant 顺序猜测终答。assistant-final 与 completion-final 重叠时，adapter 保留两条原始事实，由通用后端只投影一条对话终答，同时保留完整轨迹。Codex `turn_aborted` 转换为 `kind: "task_end", phase: "cancelled"`，完整保留 raw；后端消费该来源无关的取消事实。旧版 assistant 没有明确 phase 时保留未知，但明确的完成文本声明仍可提供终答；model/provider/phase 不猜测；只复制明确提供的 `model` 和 `model_provider`，不更名 provider 标识。

新 `token_usage_record.usage` 是带 `response_id` 的 response delta，`turn_token_usage/thread_token_usage` 的累计值保留 raw。旧 `event_msg.token_count.info.total_token_usage` 使用 `basis: "cumulative"`，`last_token_usage` 保留 raw。没有可靠 response ID 的历史累计快照只表示 session/thread 历史观察，不代表可归属的 task/response 消耗；它们保留轨迹和原始证据，不计入任务 token，也不猜测轮次增量。累计快照不能与 response delta 相加；只有可靠归属到 response 且该 response 没有 delta 时，其累计快照才能作为该 response 的用量。聚合、价格和计费由后端负责，CLI 不计算费用。

## API 与 adapter

向 `POST /api/logs/v1/events` 发送 `Authorization: Bearer <application-key>`。固定 `schema_version: "1flowbase.agent-logs/v1"`；精确 JSON 字段与示例见 [English API contract](README.md#api-envelope-and-receipt)。`task_end` 携带 `phase: "final_answer"` 和非空 content 时声明来源完成终答，`phase: "cancelled"` 表示取消且不含终答文本。kind 为 `system/user/assistant/tool_call/tool_result/usage/context/task_end`。usage 的 basis 为 `delta/cumulative`，可含 `response_id/input_tokens/output_tokens/input_cache_hit_tokens/cache_write_tokens/total_tokens`。

成功 HTTP body 沿用 `ApiSuccess<AgentLogsReceipt>`：`{"data":{"accepted_events":1,"duplicate_events":0,"record_ids":["00000000-0000-0000-0000-000000000001"]},"meta":null}`。CLI 只读取 `JSON.data` 中的 receipt，不接受顶层未包装字段。data 内 receipt 含 `accepted_events`、`duplicate_events`、UUID 字符串数组 `record_ids`；接受数与重复数之和必须等于完整批次事件数。服务端整批持久提交或整批失败；相同身份不同 payload 为冲突。应用 key 授权日志写入，不授权模型生成。

官方共享 Rust `agent-logs-collector` SDK 负责来源扫描、事件稳定身份、HTTP 上传、完整 ACK、断点排他与持久化。Codex 插件只实现来源适配接口和格式转换，直接引用主仓锁定 revision 的 canonical Rust DTO，不复制协议定义。

旧仓库 Node 采集器保留为开发 fixture oracle，不是用户安装入口。原生来源与恢复测试在官方插件仓库执行 `cargo test --locked --manifest-path sdk/agent-logs-collector/Cargo.toml` 和 `cargo test --locked --manifest-path runtime-extensions/@taichuy/codex-logs-collector/Cargo.toml`；安装 fixture 使用合成日志和本地 mock HTTP 端点。


## 采集费用估算

采集费用只按 `model_id` 精确匹配已有价格规则，`provider_code` 保留来源信息，不参与定价筛选。启用、事件时间和本地时窗均需有效；多个命中沿用价格列表的稳定顺序：供应商代码升序、优先级降序、生效时间降序、规则 ID 升序，取第一条。缺价格复用 `zero/any`，真实用量缺失时不补造费用。采集只记录估算，不扣余额；实际模型调用的供应商计费规则保持独立。

历史费用不会因价格配置变化自动重写。数据库维护者可显式运行 `agent_logs_reprice --application-id UUID --scope-id UUID`（通过私有环境变量 `API_DATABASE_URL` 提供数据库连接）。该命令按记录 ID 顺序从已保存的最小用量事实重算，仅更新费用；不重建消息或轨迹、不改事件身份/正文/Token，不重置断点或重复上传。每条记录原子提交，失败后可安全重跑。
