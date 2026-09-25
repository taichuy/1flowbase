---
memory_type: project
topic: Responses LocalSummary 与待回调历史冲突修复
summary: 用户于 2026-09-25 确认平衡方案，以新 Single Issue 修复 Codex LocalSummary 在 callback 准入前未分类而返回 409 的问题。
keywords: [Responses, LocalSummary, Codex, callback, compaction]
created_at: 2026-09-25 10
updated_at: 2026-09-25 10
decision_policy: verify_before_decision
status: active
scope:
  - api/apps/api-server/src/routes/application_public_api/openai.rs
  - api/crates/control-plane/src/application_public_api/compat/openai
---

# Responses LocalSummary 与待回调历史冲突修复

- 谁在做什么：当前开发会话按用户确认的平衡方案实现 Responses 映射协议层一次操作判定，LocalSummary 绕过 callback 关联并作为新 Generate 运行；真实 Resume 和 V2 实时 callback 优先级保持原状态契约。
- 为什么这样做：原始 Codex 自动压缩请求在识别 LocalSummary 前进入 native callback 历史证明，因历史前缀变化返回 `native_tool_output_history_mismatch`，没有创建新 flow run。
- 决策动机：维护映射协议层 → AI Native → 供应商三层边界，复用既有同 lineage 事务性 supersession，避免放宽 Resume 历史证明或增加大体量 ephemeral 缓存。
- 截止日期：用户未设定；验收以定向路由测试、真实 Resume 反例和可用环境中的真实 Codex 运行结果为准。
- 2026-09-25 10 状态：本地实现完成，定向 HTTP 回调压缩、元数据、V2 优先级与真实 Resume 历史证明测试通过，最终 `cargo check --tests` 通过；真实 Codex 降阈值验证尚未进行。本地改动未提交或推送。
- 2026-09-25 10 计划真值：已用 `gh` 创建 Single Issue [#2134](https://github.com/taichuy/1flowbase/issues/2134)，阶段为 `phase:qa`；Issue 正文包含 AC-001 至 AC-004。阶段与证据需回看当前 Issue、源码和测试产物。
