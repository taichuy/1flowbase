---
title: "Agent Flow 日志删除的账务保留决策"
memory_type: project
created_at: "2026-10-19 23"
updated_at: "2026-10-19 23"
decision_policy: verify_before_decision
status: active
tags: [agent-flow, logs, billing]
---

- 用户于 2026-10-19 明确确认单应用 Agent Flow 日志批量删除的平衡方案，并再次强调费用流水、积分流水和计费会话不可随日志级联删除；用量流水同样保留。
- Agent 负责该删除能力及验证；账务审计事实和余额属于独立生命周期，清理运行日志不构成清理账务的授权。
- 本轮测试允许复制 `1flowbase` 或 `1flowbase_gateway` 的部分开发数据；不据此扩大到生产数据或对原库执行测试删除。
- 未指定截止日期；后续同类需求先核对当前实现及用户最新授权。此条保留用户决策及动机，当前行为以应用日志 README 和源码为准。
