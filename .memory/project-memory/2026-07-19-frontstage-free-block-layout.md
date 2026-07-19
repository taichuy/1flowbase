---
memory_type: project
topic: Frontstage 区块自由组合与高度 contract
summary: Frontstage 桌面布局以 24 单位量化网格持久化自由横向组合，高度独立为 auto/fixed；移动端派生单列且不回写桌面布局。
keywords:
  - frontstage
  - block layout
  - responsive grid
  - fixed height
  - nocobase v2
match_when:
  - 调整 Frontstage 区块拖拽、缩放、响应式布局或高度配置
  - 评估自由像素画布、列网格或嵌套布局树
created_at: 2026-07-19 22
updated_at: 2026-07-19 22
last_verified_at: 2026-07-19 22
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/features/frontstage/lib/responsive-grid-layout.ts
  - web/app/src/features/frontstage/lib/page-document.ts
  - web/app/src/features/frontstage/components/PageCanvas.tsx
  - web/app/src/features/frontstage/components/jsx-studio/JsxStudioResourcePanel.tsx
---

# Frontstage 区块自由组合与高度 contract

- 谁在做什么：Frontstage 允许桌面端直接拖动区块位置和左右比例；持久化布局使用 24 单位量化网格，不保存任意像素坐标。
- 为什么这样做：用户需要一行任意组合与可调整比例，同时布局还必须可序列化、可迁移、可响应式派生。
- 为什么要做：原来的纵向排序不能表达多列 schema UI；纯像素画布又会破坏响应式确定性和长期数据演进。
- 截止日期：2026-07-19 当前 Single Issue 已实现并完成真实浏览器验收。
- 决策动机：交互自由度和存储自由度应分离。用户看到自由拖拉，系统用稳定量化 contract 保存。

冻结规则：

- `auto` 高度由内容自然撑开、页面滚动，仅允许左右 resize。
- `fixed` 高度形成内部滚动视窗，允许左右、底部和角落 resize；高度像素配置独立于布局行数。
- 移动端派生确定性单列，不回写桌面布局。
- 旧 12 单位布局迁移到 24 单位；不恢复上移/下移菜单。
- 参考 NocoBase V2 的 24 单位量化、独立高度和移动端派生原则，但当前不复制其递归 row/cell/items 布局树，也不实现 fullHeight。
