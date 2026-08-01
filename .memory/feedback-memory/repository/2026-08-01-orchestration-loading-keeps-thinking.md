---
memory_type: feedback
feedback_category: repository
topic: 编排页面初始化只保留 thinking 加载态
summary: 编排页面内部即使存在路由、应用详情和编排数据多个加载阶段，也只呈现既有 thinking，不再切换为“正在加载/打开编排”的第二段状态。
keywords:
  - orchestration loading
  - thinking
  - loading state
  - 编排页面
  - 两段式加载
match_when:
  - 修改 Agent Flow 或 Workflow 编排页面的初始化加载状态
  - 设计路由 lazy、应用详情和编辑器数据之间的用户可见过渡
created_at: 2026-08-01 00
updated_at: 2026-08-01 00
last_verified_at: 2026-08-01 00
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/pages/AgentFlowEditorPage.tsx
  - web/app/src/features/workflow/pages/WorkflowEditorPage.tsx
---

# 编排页面初始化只保留 thinking

## 规则

Agent Flow 与 Workflow 编排页面在应用详情、lazy module、编排数据、节点贡献和环境变量加载期间，统一复用既有 `LoadingState` 的 `thinking` 表现，不新增“正在加载编排”或“正在打开编排页面”等第二段用户状态。

## 原因

用户明确纠正：`thinking` 已经足够表达打开过程；把内部加载阶段依次暴露为不同文案会制造没有用户价值的两段式等待和视觉跳变。

## 适用场景

编排详情路由、编辑器页面 pending 分支、lazy fallback，以及相关加载态测试与多语言资源调整。
