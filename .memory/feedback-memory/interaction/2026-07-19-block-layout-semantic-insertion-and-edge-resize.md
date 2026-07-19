---
memory_type: feedback
feedback_category: interaction
topic: 区块布局应提供语义插入与边缘 resize
summary: 区块拖入已占满的一行时应预览并自动重分配列宽，不能要求用户先手动缩小腾空；resize 使用边缘命中区和细线反馈，不显示默认缩放图标。
keywords:
  - block layout
  - semantic insertion
  - auto repartition
  - edge resize
  - drag preview
match_when:
  - 设计或实现区块、卡片、schema UI 的拖拽组合与 resize 交互
  - 评估碰撞处理、网格压缩或插入预览
created_at: 2026-07-19 22
updated_at: 2026-07-19 22
last_verified_at: 2026-07-19 22
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - frontend builder interaction
---

# 区块布局应提供语义插入与边缘 resize

## 规则

- 用户把区块拖入已有区块所在行时，布局系统应识别“插入这一行”的意图，先展示自动划分后的稳定预览，落下后一次保存；不能要求用户先缩小两个区块再把其中一个塞进空位。
- resize 手柄应复用画布壳层式的边缘命中区：默认不显示图标，hover / active 时只强化边缘细线与 resize cursor。

## 原因

纯碰撞位移只会把区块推开或腾出空位，没有吸收用户真正想表达的“组成同一行并自动分配比例”。默认 `react-resizable` SVG 手柄也与现有画布壳层交互语言不一致。

## 适用场景

适用于 Frontstage、schema UI builder、仪表盘和其他可视化区块编排；具体分配算法仍需在每个产品 contract 中确认。
