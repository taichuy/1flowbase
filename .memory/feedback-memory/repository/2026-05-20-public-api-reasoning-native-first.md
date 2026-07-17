---
memory_type: feedback
feedback_category: repository
topic: public-api-reasoning-native-first
summary: 应用接入 API 以 1flowbase Native API / runtime event 为唯一真值层，外部映射协议必须先诚实翻译为 AI Native 再渲染到模型供应商；不得静默丢字段、把失败降级为内容或成功终态。
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
created_at: 2026-05-20 19
updated_at: 2026-07-17 15
last_verified_at: 2026-07-17 15
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

## 原因

用户纠正过：思考过程和会话能力都不是 OpenAI 专属能力，应用接入 API 应该以 1flowbase 原生接口为基础，再分别映射 OpenAI Chat Completions、OpenAI Responses 和 Anthropic。用户也明确纠正过，思考过程是结果的一部分，必须给到用户；如果只补一个兼容接口、把 `previous_response_id` 等外部协议字段当成内部真值、在兜底重建流时丢弃思考内容，或把 persisted answer 原文和 stream delta 投影混为一谈，都会造成不同接入方式行为不一致并污染状态边界。

## 适用场景

- 修改 `/api/1flowbase/runs` Native SSE。
- 修改 `/v1/chat/completions` OpenAI-compatible SSE。
- 新增或修改 `/v1/responses` OpenAI Responses-compatible 请求、响应或 SSE。
- 修改 `/v1/messages` Anthropic-compatible SSE。
- 修改应用 API 页面 / OpenAPI 文档目录，必须同步公开运行时已经支持的 Native 与兼容端点，例如模型列表、心跳和流式事件契约。
- 调整 `reasoning_delta`、`text_delta`、`thinking_delta`、`reasoning_content` 等流式事件映射。
- 若兼容协议需要 Native 当前缺失的能力，先补 1flowbase Native 真值能力，再在外层协议 adapter 投影；不要为兼容协议单独创建第二套会话、状态或事件语义。
