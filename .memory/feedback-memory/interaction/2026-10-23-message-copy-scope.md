---
memory_type: feedback
feedback_category: interaction
topic: 消息复制需求不要扩展到未发送草稿
summary: 用户要求为已有消息统一增加复制，不应自行增加输入框草稿复制。
keywords: [message, copy, scope]
match_when: [消息复制交互需求]
created_at: 2026-10-23 20
updated_at: 2026-10-23 20
decision_policy: direct_reference
---

## 规则
用户要求系统提示词、用户消息与 Agent 输出统一复制时，只覆盖已展示的消息；未明确要求时不添加未发送输入框草稿复制。

## 原因
用户已明确纠正“没有发送，不要乱加，我就要统一加一个复制而已”，原建议超出了需求范围。

## 适用场景
聊天消息复制能力补齐及同类基础交互改动；按明确对象补齐，不延伸邻近能力。
