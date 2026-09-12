---
memory_type: project
topic: Issue 2036 Responses 原生工具结果恢复原 LLM 节点
summary: 用户批准复用现有恢复机制，避免 Codex 同任务工具循环重复执行整条工作流；已创建 #2036 phase:ready，交由用户指定开发会话，本会话不实现。
keywords: [issue-2036, codex, responses, llm, resume, workflow]
match_when: [处理 Responses 工具回传、节点恢复或任务调用统计时]
created_at: 2026-09-12 17
updated_at: 2026-09-12 17
last_verified_at: 2026-09-12 17
decision_policy: verify_before_decision
---

用户于 2026-09-12 批准平衡方向，并要求新建 Issue 交给开发会话。唯一活动计划：https://github.com/taichuy/1flowbase/issues/2036 。无截止日期；开发委派与最终验收由用户进行，本会话只创建计划。

目标：Codex 三次依赖工具调用形成 1 次 workflow run、4 次逻辑 LLM 生成、3 次工具执行。前置节点一次、工具循环恢复原 LLM、最终完成后执行下游；保留原生 call_id 和每轮 response 身份，不能把日志关联当恢复授权。

原因：截图样本实际有四次完整工作流执行，虽然有 previous_response_id，运行时回调表却无记录。#2034/#2035 明确不改执行生命周期，Rounds 是展示已有运行，不能认定它引入重跑。

统计必须从正式调用事实计算，不再把 generate flow_run 数当生成次数。历史事实不改写，不以隐藏重复节点代替修复。方案与 AC、边界、迁移和恢复风险以线上 Issue 最新正文为准。
