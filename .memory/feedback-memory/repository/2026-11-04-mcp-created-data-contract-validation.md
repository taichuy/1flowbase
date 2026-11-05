---
memory_type: feedback
feedback_category: repository
topic: MCP 创建异常数据时追查完整契约与写入校验
summary: 用户要求追查 MCP 配置和校验缺口，不能以修好单个示例结束；需区分工具说明错误、后端持久化约束和前端消费约束。
keywords: [MCP, workflow, validation, persisted-data, contract]
match_when: [MCP 创建的数据能保存或调试却导致 GUI 报错, 工具说明与节点契约不一致]
created_at: "2026-11-04 17"
updated_at: "2026-11-04 17"
decision_policy: direct_reference
---

## 规则

MCP 创建的数据导致 GUI 报错时，沿 Tool 描述 / Schema → 后端写入口 → 运行时校验 → 前端消费查明断点，并只读检查同类当前数据；明确哪些是配置修正可解决、哪些是产品校验缺口。修正单个示例后不能据此宣布系统性问题消除。

## 原因

用户在修好定时示例后进一步要求检查 MCP 配置和校验为什么允许异常数据产生。已验证本次错误说明与后端漏校验同时存在，说明自然语言指导不能代替后端数据不变量。

## 适用场景

MCP / API 写入成功、节点调试成功，但正式应用 GUI 无法打开或消费结果。检查保持在当前授权领域，不据此无限扩张为全量 MCP 审计或擅自清理用户草稿和历史版本。
