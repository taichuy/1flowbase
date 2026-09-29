---
title: 存储优化监督的应用 UI 范围
date: 2026-09-29 07
decision_policy: verify_before_decision
---

用户要求当前 Root 监督存储优化在 dev 完成，功能优先且应用 UI 打开/交互正常；原 chat 无法续发时允许 tmux 新 sol/xhigh 会话。没有截止日期。动机是降低长期日志存储成本而保住应用流程、工作流、日志与详情能力。

用户于2026-09-29 07明确“UI是指应用相关工作流日志这些，不是这些demo页面”。据此验收范围覆盖真实应用编排/Workflow、日志/对话/执行追踪、节点input/debug/output与latest-active、client/Native/supplier既有读者及分页/筛选/来源跳转/恢复状态。demo不纳入本次blocker，不改变demo或运行策略；实际应用入口按owner核对，不因/route前缀排除。

此前严格全平台UI解释与demo范围待决已失效。相同集中QA补结算后应用范围无已证实集成blocker；Root完成已授权dev精确提交推送。原失败证据与有限未验证能力保持真实，不把既有缺失能力虚构为本次通过，也不重新请求范围/提交确认。

继续工作先核对当前源码/分支/运行态与最新Control Ledger，不凭此记忆直接集成。方案入口：/home/taichuy/git/1flowbase/tmp/gateway-storage-review/优化方案.md；最终交付/QA：/home/taichuy/git/1flowbase/tmp/test-governance/storage-supervision/root-final-delivery.md、central-storage-qa-application-scope.md。具体提交与测试数字查Git及回执。Gateway原409修复与存储长测证据分别归属，不能混用。

## 新数据基线（2026-09-29 16）

用户在完成 dev 存储优化后明确要求清空 1flowbase 历史，并将 dev 合并到 gateway、同等清空 1flowbase_gateway 历史，以便从零研究。Root 已完成两库会话、消息、运行日志/事件、轨迹与恢复记录清理，保留应用、工作流、模型配置、账号、密钥和余额。此授权替代旧样本保留范围；后续存储研究应使用新会话，旧数据库样本不可再用于实时查询，旧文件报告仅证明当时结果。没有另设截止日期。gateway 当前清理与运行态验收入口：/home/taichuy/git/1flowbase_gateway/tmp/test-governance/gateway-dev-merge-reset-20260929/qa/report.md；Git 状态查实际分支，不凭记忆推断。
