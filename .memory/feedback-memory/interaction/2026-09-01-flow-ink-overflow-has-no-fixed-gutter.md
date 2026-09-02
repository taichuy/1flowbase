---
memory_type: feedback
feedback_category: interaction
topic: flow-ink-overflow-has-no-fixed-gutter
summary: 当产品语义允许普通 Flow/Ink 超出区块时，不得再引入固定 gutter、padding、clip 或 allocation mode 约束越界范围。
keywords:
  - Flow
  - Ink
  - overflow
  - gutter
  - clip
  - Native Block
match_when:
  - 修复 Badge、小警告、shadow、outline 被区块边界裁切
  - 设计 Flow、Scroll、Overlay surface topology
  - 讨论是否给允许越界的区块增加固定 bleed budget
created_at: 2026-09-01 21
updated_at: 2026-09-01 21
last_verified_at: 2026-09-01 21
decision_policy: direct_reference
scope:
  - frontend architecture
  - Native Block
  - visual overflow
---

# 普通 Flow/Ink 越界不设置固定 Gutter

## 时间

`2026-09-01 21`

## 规则

- 产品已允许 Badge、小警告、shadow、outline 等普通 Flow/Ink 超出区块时，默认内容层保持 `overflow: visible`。
- 不通过固定 padding、Ink Gutter、bleed budget、`overflow: clip` 或 allocation mode 重新限制允许越界的距离。
- 横向或纵向滚动只交给有明确语义的显式 `ScrollableSurface` 或页面既有固定高度 viewport。
- 普通 Flow/Ink 可能与相邻 Block 重叠是“允许超出”的自然结果，不以禁止重叠为理由恢复裁切边界。

## 原因

固定 gutter 不能表达“允许超出区块”，只把裁切线向外移动一个常量；同时新增 DOM 层、padding、模式分支和相邻间距约束，属于没有产品收益的复杂度泄漏。

## 适用场景

- Native Block 的 Badge、状态角标、小警告、focus ring、shadow、outline、轻量 transform。
- Flow / Ink、Scroll、Overlay、Editor Chrome 分层设计。
- 任何已经明确允许普通视觉内容越出 allocation box 的前端容器。

## 例外

显式滚动区域、页面固定高度 viewport、popup Overlay 与 Editor Chrome 各自仍可拥有其语义所需的裁切、滚动或层级边界；这不构成默认 Flow/Ink 的固定 gutter。
