---
memory_type: feedback
feedback_category: repository
topic: 前端交互控件必须执行真实指针链路验收
summary: 拖拽、缩放、聚焦等交互不能用图标存在或回调装配代替验收；必须执行真实指针操作并验证运行态、持久化和控制台错误。
keywords:
  - drag
  - pointer
  - Playwright
  - interaction acceptance
  - persistence
  - pageerror
match_when:
  - 实现或验收拖拽、缩放、画布手柄、排序、聚焦等前端交互
  - UI 控件已渲染但用户反馈实际操作无效
created_at: 2026-07-19 18
updated_at: 2026-07-19 18
last_verified_at: 2026-07-19 18
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - frontend interaction QA
---

# 前端交互控件必须执行真实指针链路验收

## 时间

`2026-07-19 18`

## 规则

拖拽、缩放、聚焦、画布手柄等交互不能只验证按钮、图标、class、事件 props 或菜单存在。必须通过 Playwright 执行真实 pointer / mouse 操作，并至少检查可观察状态变化、业务副作用、刷新后持久化结果和 `pageerror`。

## 原因

Frontstage 区块工具栏曾在图标与组件测试通过时仍无法拖拽：`react-draggable` 在真实 `mousedown` 才因浏览器缺少 `process` 抛错。修复启动异常后，视觉换位和 PUT 200 仍不足以证明完成，因为保存读取了滞后的布局 ref，刷新会回退。只有完整执行“拖动 → 保存 → 刷新”才暴露并结算两个根因。

## 适用场景

适用于画布区块、节点、页面树、标签页、列表排序、resize handle，以及任何依赖真实浏览器事件序列和持久化副作用的 UI。
