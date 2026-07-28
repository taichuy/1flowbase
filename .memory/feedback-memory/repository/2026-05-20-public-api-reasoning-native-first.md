---
memory_type: feedback
feedback_category: repository
topic: public-api-reasoning-native-first
summary: 应用接入 API 以单一 1flowbase Native contract / Canonical Stream State 为唯一真值层；外部协议只存在于 adapter 边界；不得按正文去重、静默丢字段或把失败降级为成功终态。
keywords:
  - application public api
  - application api docs
  - native api
  - openai compatible
  - openai responses
  - anthropic compatible
  - reasoning_delta
  - thinking_delta
  - heartbeat
  - model list
  - honest translation
  - silent drop
  - terminal failure
  - typed operation
  - operation binding
  - canonical stream
  - lossless streaming
  - bounded backpressure
created_at: 2026-05-20 19
updated_at: 2026-07-26 09
last_verified_at: 2026-07-26 09
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/application_public_api
  - api/apps/api-server/src/application_public_docs.rs
  - api/crates/control-plane/src/orchestration_runtime
---

# Public API Reasoning Native First

## 规则

修改应用接入 API 的思考过程、流式输出、会话续接或兼容协议映射时，先确认 1flowbase Native API / runtime event stream 是否表达了真实语义；`reasoning_delta` / `<think>` 是用户可见结果的一部分，不能被当成内部噪音删除。持久化 `answer`、Native terminal snapshot 和 blocking 响应保留 `<think>...</think>` 原文；如果流式终端兜底或 completed 投影只能从持久化 `answer` 重建事件，则按 Native runtime 已有语义恢复为 `reasoning_delta` + `text_delta`，再由 OpenAI Chat Completions、OpenAI Responses、Anthropic 等兼容接口投影成各自协议字段。兼容接口只能作为 Native 请求与事件的协议投影同步维护。

协议转换固定遵循“外部映射协议 → AI Native（唯一语义真值）→ 模型供应商协议”。每个外部已提供字段都必须有明确处置：进入 Native、等价模拟、受限 transport envelope，或显式拒绝；不得静默丢弃。Provider / transport failure、`response.failed`、协议错误和缺失合法 terminal 不得写入 assistant / Answer 文本，也不得因为已有 partial answer 而渲染为正常 completed / stop；partial output 可以保留，但最终失败事实必须保持。

`Generate`、`CountTokens`、`Compact`、continuation / resource 等操作类型应作为 system-owned typed operation 进入同一套 AI Native 工作流语言，由工作流选择实际消费该操作的 LLM 节点，再由该节点对应的 Provider 插件翻译供应商 wire。操作具有 Provider-sensitive 语义，不推出入口必须在工作流执行前静态绑定 Provider；opaque payload、secret 与 continuation affinity 使用 sealed ephemeral handle 承载，不开放为普通可编辑变量。若保留 publication operation binding，它只能是由工作流编译派生的校验 / read model，不能成为与工作流并列的作者配置真值或 direct dispatch owner。

流式正文不得参与事件身份或去重。Provider frame 先由确定性 transducer 映射为 typed Native stream event，再由请求 / turn 内单写入者的 Canonical Stream State 按顺序累积；OpenAI Chat、Anthropic、Responses SSE / WebSocket 与 durable Answer Presentation 必须从同一状态派生。重复空格、换行、反引号、Markdown 或相同 chunk 都必须原样保留，不做 trim、Unicode normalization、换行转换或 live/durable prefix 补偿。除非产品明确要求跨进程 token replay，否则不建设 per-delta durable journal、下游客户端 ACK/cursor ledger或数据库轮询；必要跨 task 队列使用 bounded backpressure，正文只在等待边界或终态事务持久化。

Public API 的 `/v1`、Provider wire 版本和供应商 `/v1beta` 等属于边界协议标识，不应扩散成多套内部 canonical request/result/terminal。项目尚未形成真实兼容承诺时，直接原位重构当前 Native contract，并同步升级所有官方 Provider 消费方；不要用 V1/V3 mode、双 reader、双写或 adapter 长期维持两套语义。运行终态与 contract 版本是不同概念，不能因取消版本双栈而删除失败、未完成、取消等结果真值。

## 原因

用户纠正过：思考过程和会话能力都不是 OpenAI 专属能力，应用接入 API 应该以 1flowbase 原生接口为基础，再分别映射 OpenAI Chat Completions、OpenAI Responses 和 Anthropic。用户也明确纠正过，思考过程是结果的一部分，必须给到用户；如果只补一个兼容接口、把 `previous_response_id` 等外部协议字段当成内部真值、在兜底重建流时丢弃思考内容，或把 persisted answer 原文和 stream delta 投影混为一谈，都会造成不同接入方式行为不一致并污染状态边界。

用户进一步纠正过：为了处理 CountTokens、Remote Compact、Provider continuation / resource 和 opaque passthrough 而新增工作流外的 Provider-native operation lane，会拆出两个路由 owner。必要复杂度应由 AI Native typed operation、工作流路由、实际 LLM 节点与 Provider 插件共同承载，而不是由入口预检提前替工作流选择 Provider。

用户还纠正过：这里的“消费”是 Gateway 消费 Provider 流并写入下游 response writer，不是维护终端客户端 ACK。当前缺片根因是正文 HashSet 去重与 live/durable 双真值，不应扩大成 durable event sourcing 或 delivery ledger。

## 适用场景

- 修改 `/api/1flowbase/runs` Native SSE。
- 修改 `/v1/chat/completions` OpenAI-compatible SSE。
- 新增或修改 `/v1/responses` OpenAI Responses-compatible 请求、响应或 SSE。
- 修改 `/v1/messages` Anthropic-compatible SSE。
- 修改应用 API 页面 / OpenAPI 文档目录，必须同步公开运行时已经支持的 Native 与兼容端点，例如模型列表、心跳和流式事件契约。
- 调整 `reasoning_delta`、`text_delta`、`thinking_delta`、`reasoning_content` 等流式事件映射。
- 若兼容协议需要 Native 当前缺失的能力，先补 1flowbase Native 真值能力，再在外层协议 adapter 投影；不要为兼容协议单独创建第二套会话、状态或事件语义。
