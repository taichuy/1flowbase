---
memory_type: feedback
feedback_category: interaction
topic: oauth-task-scope
summary: OAuth 接入任务只处理已批准范围，不顺手修改通用页签名称。
created_at: "2026-11-16 12"
updated_at: "2026-11-16 12"
decision_policy: direct_reference
scope: [mcp-client-oauth]
---

## 规则

OAuth 接入任务保留现有“通用”页签名称；只增加已批准的 OAuth 页签及必要后端协议支持。

## 原因

用户明确纠正了将“通用”改名为 API key 的建议，要求聚焦 OAuth，不做无关事情。

## 适用场景

本次 MCP OAuth 实现、审查和验收。其他既有界面的改名不从本次授权推导。
