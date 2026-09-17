---
memory_type: project
topic: Issue 2062 Responses call_id Provider 边界一致性
summary: 用户批准停止在 Responses 输出中拼接 callback task UUID，并在所有 resume 模式的 Provider 出口兼容清理旧 calltask_ 历史；实现等待重启后的真实 Codex 验收。
keywords: [issue-2062, codex, responses, call-id, callback, provider-transport]
match_when: [处理 Responses call_id 超长、calltask_、native callback 或 mixed transport history 时]
created_at: 2026-09-16 14
updated_at: 2026-09-16 14
last_verified_at: 2026-09-16 14
decision_policy: verify_before_decision
---

用户于 2026-09-16 批准 Issue #2062，目标是在当天修复真实运行 `01a0a89a-c92c-7360-bd0b-2f1b6ebe4802` 的 71 字符 `call_id` 上游拒绝。

边界决策：OpenAI Responses 对外投影保留 Provider 原始 `call_id`，durable native callback state 负责回调关联；网关内部 callback task UUID 不再拼进新的 Responses `call_id`。历史会话中的 `calltask_{uuid}_{provider_call_id}` 继续兼容，但无论当前回调由 encoded state 还是 native state 关联，都必须在 Provider transport 边界还原，不能进入供应商请求。OpenAI Chat 的既有编码不属于本 Issue。

实现覆盖 blocking、HTTP/SSE 与 WebSocket Responses 投影的同一语义，并增加旧 encoded history 与当前 native custom-tool callback 混合回归。首次 `api-server` 测试链接两次超过 16 分钟且占用约 9GB 内存后按资源停止；`cargo check -p api-server --tests` 已通过，真实 Codex 行为仍需用户重启 7800 后验证。
