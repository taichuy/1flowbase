---
memory_type: project
topic: 应用管理批量删除采用逐项复用接口方案
summary: 用户确认在新增与导入之间增加删除数量按钮，复用单应用删除接口，允许部分成功，失败项保留选择供重试。
created_at: 2026-09-11 08
updated_at: 2026-09-11 08
decision_policy: verify_before_decision
keywords: [application-management, batch-delete, permissions]
---

用户在本会话确认保守方案，Codex 实施 /settings/applications 批量删除，以减少逐行删除操作。此次范围为前端工具栏、确认、权限和结果反馈；不新增批量后端接口或回收站。原因是现有单项删除能力足够覆盖需求，逐项结果允许小范围交付。截止口径为本次用户验收完成；后续扩大到事务原子删除或恢复能力时重新对齐。验收证据见 tmp/test-governance/application-batch-delete/qa.md，核心场景已通过，尚未替代用户最终验收。
