---
memory_type: project
topic: application-public-api-decisions
summary: 应用公开调用 API 方向确认：Key 只调用已发布版本；Native canonical 路由为 `/api/agent/v1/runs` 与 `/api/agent/v1/files`，OpenAI `/v1/chat/completions`、OpenAI Responses `/v1/responses`、Anthropic `/v1/messages`；Key 绑定应用和创建人，用户仅看自己创建的 Key；Native payload 固定使用 `query/model/history`，其中 `model` 是可选字符串且不做值校验；public Native resume 第一版采用 API 进程内 worker。
keywords:
  - application api
  - application api key
  - public route
  - native run api
  - openai compatible
  - anthropic compatible
  - anthropic 1m context
  - streaming
  - terminal answer fallback
  - public native resume
  - resume worker
created_at: 2026-05-09 23
updated_at: 2026-07-23 17
last_verified_at: 2026-07-23 17
decision_policy: verify_before_decision
scope:
  - api
  - web/app/src/features/applications
  - web/app/src/features/settings/components/ApiDocsPanel.tsx
---

# Application Public API Decisions

## 谁在做什么？

用户正在确认 1flowbase 应用级公开调用 API 的产品与协议方向，AI 后续需要基于这些确认输出设计文档和 implementation plan。

## 为什么这样做？

公开调用应面向“应用能力”而不是暴露具体 application id。API Key 绑定应用和创建人，外部路由统一不变，由 Key 鉴别进入哪个应用；Key 调用必须走已发布版本，不能调用编辑草稿。

## 为什么要做？

AgentFlow 已进入调试跑通阶段，下一阶段最重要业务是应用 API：生成应用 API Key、生成原生工作流接口、并将原生接口映射为 OpenAI `/v1/chat/completions` 与 Anthropic `/v1/messages` 兼容接口。文档组件复用 `/settings/docs` 的 Scalar/OpenAPI 能力，但应用详情内需要做面向当前应用的 API 组件，不跳转到全局文档页。

## 截止日期？

无固定截止日期；后续设计文档和计划应优先引用本决策。

## 决策背后动机？

- 应用 API Key：不限制数量；只做创建、列表、删除；绑定创建人；每个人仅能看到自己创建的 Key。
- 路由：Native canonical 对外路径为 `POST /api/agent/v1/runs`、`GET /api/agent/v1/runs/{run_id}`、`POST /api/agent/v1/runs/{run_id}/cancel`、`POST /api/agent/v1/runs/{run_id}/resume`、`POST /api/agent/v1/files`，所有应用类型共用；OpenAI 兼容 `/v1/chat/completions`；Anthropic 兼容 `/v1/messages`。
- `2026-05-30 10` 用户确认项目初期没有发布，旧 `/api/v1/agent/...` 直接下线，不保留兼容入口；#541 实现新 canonical path。
- `2026-05-22 15` 用户确认移除冗余 OpenAI alias `/openai/v1/chat/completions`；未对用户开放且没有使用者，公开文档和后端路由都只保留 canonical `/v1/chat/completions`。
- `2026-05-26 18` 用户确认 OpenAI Responses API 也属于应用兼容投影层；主仓 Native/runtime 是唯一真值，Responses/OpenAI Chat/Anthropic 只从 Native 事件和 durable answer 投影协议形状。若 runtime 终态是 `flow_failed` 但 durable `run.answer` 已存在，兼容 SSE 必须把可用 answer 完整输出并发送协议完成事件，不能让后续失败节点或工具回调失败吞掉前面已完成 answer。OpenAI Responses 根路径 `/responses` 与 `/v1/responses` 都应支持 plain base URL 客户端。
- `2026-05-30 23` 用户再次确认三层边界：对外 OpenAI / OpenAI Responses / Anthropic 等协议只是 1flowbase Native application run 的外部投影；1flowbase Native/run event/Answer Presentation 是唯一真值层；供应商 wire shape 只由对应 provider 插件适配。网关协议转换可以参考 LiteLLM 等成熟项目，但不能让外部协议成为内部真值。
- `2026-06-02 23` 用户确认 #611 public Native resume 的 worker 部署第一版走 API 进程内启动，不单独开 resume-worker 容器或独立服务；动机是当前阶段进程挂了通常服务也不可用，独立容器会增加资源与部署成本。持久化层仍应保存 resume request / claim / lease 事实，ephemeral 层最多做唤醒信号，不作为唯一队列。
- `2026-07-03 00` 用户确认 Claude Code / Anthropic 兼容中的 `[1M]` 只属于入口协议扩展：Native API 仍是唯一真值，Native `model` 保持标准模型 ID；Anthropic mapper 可以识别尾部 `[1M]` 并转换为 `anthropic-beta: context-1m-2025-08-07` / `client_protocol_envelope`。具体 provider / plugin 不承载该语义；是否 1M 由应用 start node `model_list.context_window` 等 Native 模型配置表达。
- `2026-07-16 08` 用户再次确认模型调用必须坚持 `Anthropic -> AI Native -> Anthropic`：AI Native 是唯一执行真值，入口 Anthropic DTO 与供应商 Anthropic wire shape 都只是边界投影；修复协议兼容问题不得把原始请求正文作为第二真值透传，必须先提升为 AI Native 语义，再由 provider 插件确定性渲染。
- `2026-07-16 16` 该方向已完成 callback 闭环修复：Provider 首轮调用前冻结实际 AI Native `system/messages` transcript，内部工具和外部 callback 只在 transcript 后追加 assistant tool call / tool result；Answer Presentation 完成态按 `answer_node_id + segment_index + source_node_id + source_node_run_id + output_key` 补缺，不再按整条 run 文本猜前缀。真实 Claude Code 经工具 callback 后保留原始上下文，最终 Anthropic SSE 文本只出现一次。
- `2026-07-23 17` 用户选择兼容流式入口的激进长期方向，并批准线上 Root #1440：Anthropic Messages、OpenAI Chat Completions 与 OpenAI Responses 的全部有状态 `stream:true` Start / Resume 统一进入一个 Compatible Turn 流生命周期；`orchestration-runtime` 与 `RuntimeEventStream` 继续分别作为唯一执行状态机和实时事件源，不恢复数据库轮询。路由只解析、鉴权和形成 `Start | Resume` typed action；统一流模块必须在后台执行前完成订阅，并负责 cursor、已消费 callback、sender 生命周期、typed closure、至多一次 durable reconcile 与协议终态。三种 wire mapper 保持独立，只共享 typed runtime-event view 和 exactly-once 终态约束；删除有状态生产 `completed_*_stream` 分叉。OpenAI Compact 是不创建 flow run 的 unary operation，保留单事件 SSE 但改成明确的 Compact 命名。验收使用本地 Claude Code、Codex 源码客户端与 OpenCode 源码客户端，经 tmux + PTY timing 证明增量输出，并覆盖工具回调、幂等重放、新问题新 run、截断、错误、断连与 `stream:false`。
- Native payload 不能只做 `inputs/response_mode/user/metadata`，必须重新设计，覆盖 query、文件图片、会话绑定、本地 agent 工具回调、流式、协议映射预留。
- Native API 标准字段固定为 `query` 表达当前轮输入、`history` 表达外部上下文；OpenAI/Anthropic 兼容层都映射到这套 Native envelope，不在 Native API 里直接使用兼容协议的 `messages` 作为主结构。
- Native API 增加与 `query` 同级的可选 `model` 字符串字段；平台只校验它是字符串，不校验值、不按它路由、不要求它匹配公开 serving id。后续节点怎么使用 `model` 由应用编排和 mapping 自己配置。
- 文件输入第一版以 `upload_file_id` 为主；`url/base64` 作为协议预留，兼容层接到远程 URL 或 base64 时优先转换成内部 file record 后再进入 `attachments`。
- 兼容协议第一版可以只支持 text chat，但需要为 tools / files / images 等下一步计划预留；不兼容功能必须在 API 文档中写明，调用方可基于 Native API 自己转换或通过插件扩展。
- Streaming 是核心能力，第一版必须做。
