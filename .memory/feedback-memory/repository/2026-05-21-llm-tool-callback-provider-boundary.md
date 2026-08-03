---
memory_type: feedback
feedback_category: repository
topic: llm-tool-callback-provider-boundary
summary: 1flowbase 运行时/Native 形状是 provider 唯一真值层；供应商 wire shape 只能由 provider 插件源码适配，plugin host 不替插件改写 tool_calls、私有协议事件或 WebSocket/SSE 传输降级策略。
keywords:
  - llm tool callback
  - tool_calls
  - ProviderInvocationInput
  - provider host
  - native truth
  - openai compatible
  - openai responses
  - websocket fallback
  - retry policy
created_at: 2026-05-21 17
updated_at: 2026-08-03 23
last_verified_at: 2026-08-03 23
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - api/crates/plugin-framework/src/provider_contract.rs
  - api/apps/plugin-runner/src/provider_host.rs
  - api/apps/api-server/src/provider_runtime.rs
  - api/crates/orchestration-runtime/src/execution_engine
  - ../1flowbase-official-plugins/runtime-extensions/model-providers/anthropic
  - ../1flowbase-official-plugins/runtime-extensions/model-providers/openai
---

# LLM Tool Callback Provider Boundary

## 规则

处理 LLM tool callback、assistant `tool_calls` 历史、tool result continuation 或 provider stream terminal event 时，运行时与 1flowbase Native API 保持中立 `ProviderInvocationInput` / `ProviderToolCall` / `ProviderStreamEvent` 语义作为唯一真值。`plugin-runner` / provider host 只把 1flowbase native truth 交给 provider 插件，不替插件改写成某个供应商 wire shape 或传输策略。外部供应商协议需要的字段形状，例如 OpenAI-compatible 的 `type:function` + `function.arguments` 字符串、OpenAI Responses WebSocket 的 `response.done` 终止事件，或 WebSocket close 后是否降级到 HTTP SSE，由对应 provider 插件源码自己适配。

OpenAI Responses WebSocket 中，`response.created` 只代表上游生命周期开始，不代表已经向 1flowbase runtime 输出了可见内容；它不能单独阻断 HTTP SSE fallback。只有已经 emit 过 text/reasoning/tool 可见事件，或收到 `response.failed` / `response.incomplete` / failed `response.done` 这类语义失败时，才应禁止 fallback，避免重复输出或吞掉真实供应商错误。

OpenAI Responses tool callback continuation 中，`previous_response_id` 是 1flowbase Native provider contract 传给插件的 cursor，不是所有上游 transport 都能原样消费的公共字段。插件必须按 response_id 来源处理：如果该 response_id 是本 worker 通过 Responses WebSocket 拿到的，后续 continuation 应重连 WebSocket 一次，不能把这个 cursor 降级到 HTTP SSE；如果从未见过该 WebSocket response_id，则允许 HTTP-only 兼容入口按自己的 `/responses` 语义处理。

LLM tool callback continuation 为节省上下文只向 provider 发送 tool result delta messages 时，`ProviderInvocationInput.system` 仍属于 1flowbase Native provider contract 的稳定真值，必须从 checkpoint history 保留下来。OpenAI Responses 插件再把这个 native `system` 映射为上游 `instructions`；不要在主仓运行时、`api-server` 或 `plugin-runner` 通用 host 中引入 OpenAI 专属字段。

供应商协议序列化错误由对应 provider 插件修复；调用失败后的重试分类、次数和退避策略属于 Agent Flow 大语言模型节点专门模块。修复 provider wire shape 时不得顺手修改重试策略；即使发现确定性 `4xx` 被重复调用，也应作为独立的大语言模型节点任务处理。

AI Native `reasoning` 映射到供应商 wire 时，provider 插件应保留可表达的常用推理强度；例如 Chat-compatible 或 DeepSeek 无法表达 `mode=adaptive`、但能表达 `effort=high` 时，继续发送供应商 `reasoning_effort=high`，并在 `provider_metadata.provider_request_translation.decisions` 记录被省略的语义。完全无法表达时省略并记录，不得仅因部分语义不可表达就在 HTTP 前拒绝整个调用；节点显式供应商参数优先，真正不支持的 wire 值交由上游明确返回。

## 原因

用户纠正过：1flowbase 只有一个唯一真值层，把 OpenAI 形状补在 `api-server`、运行时输入或 `plugin-runner` 通用 host 里，只解决了一个协议的表象，还会让兼容协议污染 Native 真值。真正问题是工具回调或 provider stream 跨越运行时真值和供应商线协议时缺少清晰边界。

## 适用场景

- 修改 `ProviderInvocationInput.messages[].tool_calls`、`ProviderToolCall`、LLM 节点输出或 continuation history。
- 修复供应商返回 `missing field type`、`function.arguments` 类型不匹配等协议反序列化错误。
- 修复 OpenAI Responses API 插件的 SSE / WebSocket 事件名、终止事件、tool call 或 tool result 映射。
- 修复 OpenAI Responses API 插件的 WebSocket/SSE 传输降级，尤其是 `response.created` 后无可见输出即 close 的场景。
- 修复 OpenAI Responses API 插件携带 `previous_response_id` 的工具回调续跑，尤其是避免 WebSocket response cursor 被 fallback 成 HTTP SSE 后触发 `previous_response_id is only supported on Responses WebSocket v2`。
- 修复工具结果续调后上游报 `Instructions are required`：先检查 Native `ProviderInvocationInput.system` 是否在 `previous_response_id + delta messages` 路径中被保留，再由对应插件转换成供应商字段。
- 新增非 OpenAI provider 协议适配时，优先在 provider 插件源码处理，不改 `plugin-runner` 通用 host、运行时 checkpoint、Native public API 或 api-server 通用 runtime port。
- 处理 provider `4xx/5xx`、空响应或首次 token 后失败的重试行为时，进入大语言模型节点专门模块，不在 provider 插件协议修复中混改重试策略。
