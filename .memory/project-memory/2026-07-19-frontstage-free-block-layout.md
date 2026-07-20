---
memory_type: project
topic: Frontstage 区块自由组合与高度 contract
summary: Frontstage 以共享 24 单位网格内核承载自动布局与自由网格两种文档级策略，高度独立为 auto/fixed；移动端派生单列且不回写桌面布局。
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
updated_at: 2026-07-20 08
last_verified_at: 2026-07-20 08
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

- Page/Tab Document 的 `layoutMode` 取值为 `auto / free`，缺省与新文档默认 `auto`；页面配置使用“布局方式”下拉切换。
- `auto` 强制受影响行连续铺满：拖动同时重组来源行与目标行，空白行单区块铺满，相邻 resize 边界联动调整比例。
- `free` 保留独立 `x/y/w/h` 并允许空隙；两种策略共享 bounds、min/max、collision、responsive 与 commit 内核，不复制 PageCanvas。
- `auto` 高度由内容自然撑开、页面滚动，仅允许左右 resize。
- `fixed` 高度形成内部滚动视窗，允许左右、底部和角落 resize；高度像素配置独立于布局行数。
- 纵向碰撞网格与视觉间距解耦：RGL 使用 3px 整数行、0 内部 vertical margin，区块占位预留 10px；自然高度量化误差不超过 2px，桌面与 390px 设计态实测可见间距约 10.4px。
- 持久化布局写入 `verticalGridVersion: 2`；旧纵向行坐标按原 44px 节拍换算为像素等价的新坐标，避免历史页面刷新后上移或重叠。
- PageCanvas 的宽度测量宿主在空状态和非空状态间保持同一 DOM 节点，确保 ResizeObserver 在“空页面 → 首个区块”之前已经挂载；禁止用默认 1280px 或创建后手动测量替代稳定宿主生命周期。
- 移动端派生确定性单列，不回写桌面布局。
- 旧 12 单位布局迁移到 24 单位；不恢复上移/下移菜单。
- 参考 NocoBase V2 的 24 单位量化、独立高度和移动端派生原则，但当前不复制其递归 row/cell/items 布局树，也不实现 fullHeight。

## 后续演进真值

- 在线 Single Issue：[#1376 建立 Frontstage 可演进区块布局内核与连续碰撞交互](https://github.com/taichuy/1flowbase/issues/1376)
- 当前阶段：`phase:user-acceptance`；自动/自由策略、RGL v2 public API、确定性行接触 solver、边缘 resize、no-op save 与桌面/移动端真实指针验收已完成。
- 只修改 1flowbase；`/home/taichuy/git/react-grid-layout` 仅作 `2.2.3` 参考源码，不修改、fork、patch 或本地链接。
- 后续交互采用连续像素 preview 与响应式网格 commit 双态模型；24 列是 Frontstage desktop profile，不是通用布局内核常量。
