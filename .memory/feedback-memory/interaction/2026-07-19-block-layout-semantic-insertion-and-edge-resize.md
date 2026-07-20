---
memory_type: feedback
feedback_category: interaction
topic: 基础区块布局应由长期可演进的几何求解内核承载
summary: 区块拖入已占满的一行时应通过成熟几何、约束和碰撞响应产生自动重排预览，不能要求用户先手动腾空，也不能用局部分栏 heuristic 或 CSS 效果代替底层布局设计。
keywords:
  - block layout
  - semantic insertion
  - auto repartition
  - edge resize
  - drag preview
  - collision solver
  - react-grid-layout
match_when:
  - 设计或实现区块、卡片、schema UI 的拖拽组合与 resize 交互
  - 评估碰撞处理、网格压缩或插入预览
created_at: 2026-07-19 22
updated_at: 2026-07-20 09
last_verified_at: 2026-07-20 09
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - frontend builder interaction
---

# 区块布局应提供语义插入与边缘 resize

## 规则

- 用户把区块拖入已有区块所在行时，布局系统应识别“插入这一行”的意图，先展示自动划分后的稳定预览，落下后一次保存；不能要求用户先缩小两个区块再把其中一个塞进空位。
- resize 手柄应复用画布壳层式的边缘命中区：默认不显示图标，hover / active 时只强化边缘细线与 resize cursor。
- resize 的命中几何与视觉几何必须分离：左右边保留全高透明命中区，可见提示只使用居中的短线，不能把全高命中区绘制成长边线或突出区块上下边界。
- 区块布局属于初始化阶段的基础能力，方案必须说明长期 geometry、collision、constraint、compaction、responsive 与 persistence 边界；不得用只解决当前两个区块的分栏规则冒充布局架构。
- 碰撞与尺寸响应使用纯计算内核和成熟算法；CSS 只呈现求解结果与命中反馈，不拥有布局真值或碰撞逻辑。
- `/home/taichuy/git/react-grid-layout` 默认只作为已发布依赖的参考源码；优先通过包公开的 `core / react / extras` API 在 1flowbase 内组合。未经用户另行授权，不修改、fork 或 patch 上游源码。
- 自动铺满与允许空隙是两个不同产品语义，必须显式建模为可持久化布局策略；共享 geometry/constraint/commit 内核，不能靠同一算法在不同拖拽场景中隐式切换心智。

## 原因

纯碰撞位移只会把区块推开或腾出空位，没有吸收用户真正想表达的“组成同一行并自动分配比例”。仅在 Frontstage 叠加局部分栏 planner 会固化短期产品语义；应先利用 `react-grid-layout` 已发布的底层扩展面，再由 1flowbase 拥有产品布局策略。默认 `react-resizable` SVG 手柄也与现有画布壳层交互语言不一致。

## 适用场景

适用于 Frontstage、schema UI builder、仪表盘和其他可视化区块编排；具体分配算法仍需在每个产品 contract 中确认。
