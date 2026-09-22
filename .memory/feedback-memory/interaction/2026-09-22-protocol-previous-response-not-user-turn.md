---
date: 2026-09-22 18
feedback_category: interaction
decision_policy: direct_reference
---

# 区分协议前序请求与用户对话轮次

- 规则：看到 `previous_response_id` 时先查它指向的请求种类，不直接称为“上一轮用户对话”。
- 原因：用户指出新开会话也出现该字段；本次数据库证实前序请求为 `request_kind=prewarm`、`generate=false`、无用户 prompt 或 turn_id。
- 适用场景：解释会话续接、预热、恢复与模型日志关联时，区分后台协议请求与用户可见对话。
