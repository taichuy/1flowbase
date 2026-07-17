---
memory_type: feedback
feedback_category: repository
topic: 应用日志用户输入边界
summary: 应用日志按真实 conversation id 展示完整 flow_run 对话；客户端真实发送的文本和框架 meta 文本不得隐藏或清洗，工具注册与结构化兼容协议上下文仍归入运行追踪。
keywords:
  - application logs
  - run detail
  - user input
  - start node input
  - start input artifact
  - history
  - tool schema
  - tools
  - conversation pagination
match_when:
  - 修改应用日志、运行详情、公开 API 兼容请求映射或 debug artifact 预览时
created_at: 2026-05-19 21
updated_at: 2026-07-17 17
last_verified_at: 2026-07-17 17
decision_policy: direct_reference
scope:
  - web/app/src/features/applications
  - api/apps/api-server/src/routes/applications/application_runtime
  - api/crates/control-plane/src/application_public_api
---

# 应用日志用户输入边界

## 时间

`2026-05-19 21`

## 规则

应用日志聊天视图中的用户消息只代表用户原始发言。运行详情顶层字段使用 `query` / `model` 这类业务命名，不使用 `input_text` / `input_model`；历史对话按真实 `external_conversation_id` 分页读取，并以当前 run 为锚点加载附近消息。当前 run 的 `input_payload.history` / `input_payload.messages` 是模型输入上下文快照，不是平台会话状态；有真实会话 id 时不能用它替代同会话 run 分页，但没有会话 id 时可以作为 imported context fallback 展示，且这些上下文消息必须 `can_open_detail=false`、`detail_run_id=null`。如果这些输入上下文被 runtime debug artifact 截断或 offload，后端可为还原本次输入上下文读取 artifact，但仍不能把它们标成真实运行历史。工具注册统一归一为 start 节点输入里的稳定变量，例如 `userinput.tools` 和 `userinput.tool_choice`；`userinput.history` / `userinput.tools` 这类数组变量如果被 runtime debug artifact 截断，调试变量面板必须加载完整 artifact 后再投影，不能只依赖 start 节点直接展开出来的 `query` / `model` / `files` 摘要；start 节点输出在日志语义上保持空对象。工具调用、兼容协议透传字段、运行参数和大 payload 预览应作为 start 节点输入、节点详情或运行追踪记录，不应被当作用户聊天内容展示。

客户端真实发送的文本块必须完整落库并可见，包括 Claude Code 等客户端附加的 retry / continuation meta 文本，例如 `[Your previous response had no visible output. Please continue and produce a user-visible response.]`。平台可以标注来源或在结构上区分用户手输文本与客户端框架文本，但不得为了界面简洁隐藏、删除、改写或清洗；用户直接核对原始对话时必须能还原当时的完整请求。

## 原因

用户打开聊天记录时预期看到的是自己说的一句话和对应上下文。真实会话 id 存在时，历史应来自真实 run；没有会话 id 时，调用方传入的 history 是唯一可见上下文，应该展示但不能伪装成可下钻的运行历史。把工具 schema 或 compatibility payload 放进用户气泡会误导排查，也会把开始节点输入、模型上下文和平台会话分页三个职责混在一起。

## 适用场景

修改应用日志、运行详情、公开 API OpenAI / Anthropic 兼容映射、debug artifact offload、start 节点输入展示、对话日志抽取逻辑时命中。后端应提供用于展示的用户输入真值字段和规整后的工具注册变量，前端优先消费后端字段，不递归猜测工具 schema。
