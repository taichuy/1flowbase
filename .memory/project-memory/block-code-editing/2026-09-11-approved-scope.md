---
date: 2026-09-11 00
decision_policy: verify_before_decision
subject: 区块源码局部编辑接口及 MCP 配置范围
---

用户已确认完善后端接口及对应 MCP 配置，并要求创建 Single Issue；已创建 https://github.com/taichuy/1flowbase/issues/2027 ，为唯一活动计划真值。
动机：避免 Agent 修改少量通知时因只能发现全文保存工具而中止，支持长区块局部定位与编辑。
范围：复用已有片段/行列编辑，补源码搜索、精确文本替换和有限结果返回，同步 MCP 工具及映射。明确不做多文件、语法树编辑、自动合并或发布。
用户随后明确授权开始实现，并确认纳入安全的 MCP 结构化错误诊断。Codex 已完成接口及开发环境 MCP 配置，定向自动化、真实 MCP 并发/错误恢复与通知 fixture 页面验收通过，现进入用户验收；后续状态以 issue 为准。无约定截止日期。
