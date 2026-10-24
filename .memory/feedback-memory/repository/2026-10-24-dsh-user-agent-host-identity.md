---
date: 2026-10-24 00
feedback_category: repository
decision_policy: direct_reference
scope: 1flowbase-dsh-plugin upstream User-Agent
---
# DSH host identity

规则：DSH 插件请求上游时，User-Agent 使用实际 DeepSeek Harness 身份；按 product/version (OS version; architecture) 格式生成，并复用宿主版本来源。不要用插件名替代宿主，也不要默认伪装 Codex Desktop。
原因：用户纠正“我们只是插件而已”，并在两种解释中明确选择“按格式生成 DeepSeek-harness 的真实信息”。
适用：本插件 Chat、Responses（WebSocket/SSE/compact）、Anthropic 请求归属；后续用户明确指定身份覆盖时重新按其要求处理。
