---
created_at: 2026-08-05 13
memory_type: feedback
feedback_category: repository
decision_policy: direct_reference
scope: cross-cutting quality gate workflow integration
---

# Cross-Cutting Gates Must Join The Unified Quality Aggregate

用户在 2026-08-05 纠正：四基座门禁不应只作为独立 GitHub Actions workflow 存在，必须放进现有质量门禁。

规则：新增跨模块 QA workflow 可以作为可复用执行单元，但 PR/push 应由现有 `verify` 调用，nightly/manual 应由现有 `quality gate` 调用；其标准 component report 必须进入统一 aggregate 与 expected scopes。独立 `workflow_dispatch` 只用于聚焦诊断，不能代替质量门禁集成。

原因：产物落在 `tmp/test-governance/` 不等于已经进入质量门禁。若没有被统一 workflow 调度、没有标准 component report、没有进入 aggregate，管理员仍需查看第二套旁路状态，lane、报告和失败归因会重新分裂。

适用场景：新增 foundation contract、security、coverage、hygiene、consistency 或其他横跨多个模块的 GitHub Actions 质量证据时。
