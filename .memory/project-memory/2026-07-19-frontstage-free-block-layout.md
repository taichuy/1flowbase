---
memory_type: project
topic: Frontstage 区块自由组合与高度 contract
summary: Frontstage 以共享 24 单位网格内核承载自动布局与自由网格两种文档级策略；自动布局拖拽使用 pointer midpoint、0.5 列 deadband 与 stable insertion，首/中/末位均可达。
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
updated_at: 2026-08-27 19
last_verified_at: 2026-08-27 19
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/features/frontstage/lib/responsive-grid-layout.ts
  - web/app/src/features/frontstage/lib/page-canvas/frontstage-block-interaction.ts
  - web/app/src/features/frontstage/lib/page-document.ts
  - web/app/src/features/frontstage/components/PageCanvas.tsx
  - web/app/src/features/frontstage/components/jsx-studio/JsxStudioResourcePanel.tsx
  - web/patches/react-grid-layout@2.2.3.patch
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
- `auto` 的排序意图由连续 pointer column 相对目标 midpoint 决定；midpoint 两侧使用 0.5 列 deadband，区间内保持上一 stable insertion。没有连续 pointer 的首帧以水平移动方向消除中心相等偏置，不能用第一列特判。
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
- 首位插入修复：[#1897 修复自动布局拖拽首位插入不可达](https://github.com/taichuy/1flowbase/issues/1897)，commit `d7d8e3288` 已推送 `dev`，用户于 2026-08-26 验收并关闭。
- 边缘滚动 Issue：[#1899 支持区块拖拽边缘自动滚动](https://github.com/taichuy/1flowbase/issues/1899) 已由用户于 2026-08-27 验收并关闭。1flowbase 通过精确绑定 `react-grid-layout@2.2.3` 的 pnpm dependency patch，在 `GridItem` 私有坐标 owner 内用 smoothstep rAF、6px 单帧上限、drag-start 最大滚动快照和实际 scroll delta 补偿保持 dragged item / placeholder 同步；双向 Playwright 最大漂移 6px、一次拖拽一次保存、刷新持久化与 #1897 回归均通过。
- 二维投影 Issue：[#1900 支持并排行与独占行之间的二维拖拽投影](https://github.com/taichuy/1flowbase/issues/1900) 已由用户于 2026-08-27 验收并关闭。自动布局 drag session 使用缓存的逻辑行序列与二维 intent classifier，互斥表达 `JoinRow(rowIndex, cellIndex)` 和 `InsertStandaloneRow(rowIndex)`；纵向边界采用进入／退出不同阈值的滞回，行位置由成员最大高度前缀和生成。PageCanvas 提供 content-space pointer，compactor 缓存 pointer-to-active offset 以兼容 #1899 无 pointermove 的边缘滚动；不改变持久化布局 contract 或 RGL patch。
- 当前活动 Issue：[#1902 自动布局按行最大内容高度统一区块外框](https://github.com/taichuy/1flowbase/issues/1902) 已按用户确认升级为 `grade:g4`。Frontstage 通过 `ctx.ui.sizing` 建立双通道高度 contract：宿主向 Block 提供 `available` width/height，Block 用 `reportIntrinsicSize` 独立报告自然需求；显式报告后高度链贯通 intrinsic wrapper、content viewport、native root 与 ShadowRoot mount，但 observer 不再把 allocated height 反写 intrinsic demand。runtime identity 作为显式 sizing owner，源码换代时不消费 contract 的旧 Block 自动恢复自然高度，尺寸更新不 remount。
- 用户 Block `01a04127-cbdb-7e70-a723-f8c74fd9b966` 已由 MCP 从 SHA `e7a02164...33cb` 迁移到 `32afd8d6...6fb7`，自然高度为 `392px` 并消费 runtime available height。真实页面 1600px 桌面证据：同行 slot `926px`、intrinsic wrapper/content viewport `924px`、内部 host `900px`（扣除 12px 双侧 padding），generation 保持 `0`，console/page errors 为 0；截图与证据位于 `tmp/test-governance/issue-1902-viewport-contract/`。
- 当前阶段：`phase:user-acceptance`；自动/自由策略、RGL v2 public API、确定性行接触 solver、边缘 resize、no-op save、二维投影、行级外框等高及 Block runtime available-size 传播已完成，等待用户最终验收 #1902。
- `antd-style` ShadowRoot Issue：[#1907 修复 antd-style ShadowRoot 样式隔离并按需加载](https://github.com/taichuy/1flowbase/issues/1907) 已实现并进入 `phase:user-acceptance`。Native React 只为实际导入 `antd-style` 的 Block 建立 per-surface ShadowRoot `StyleProvider` 与唯一 Emotion prefix；registry 改为 module-level dynamic import 并复用 single-flight / 浏览器 ESM cache，production chunk 约 `17.5 KB / 6.43 KB gzip`。真实 Block `01a04264-25ec-7f72-abeb-cba94795817d` source SHA 保持 `e338479e32fae7600d8a1b0d86e555996bcd69bcc058b446fad3c3d2cc5077f1`，computed style、ShadowRoot 规则归属、定向测试、TypeScript、production build 与 `page.frontstage` style-boundary 均通过；证据位于 `tmp/test-governance/issue-1907-antd-style/`。
- Native Block Anchor Issue：[#1910 修复 Native Block Anchor 的 ShadowRoot 目标解析与滚动归属](https://github.com/taichuy/1flowbase/issues/1910) 已完成实现与两轮 Dev Acceptance Gate，待用户再次验收。runtime surface 将 ShadowRoot `targetRoot` 与真实 `scrollOwner` 分离，注入的 `antd.Anchor` 只在当前 ShadowRoot 解析本地目标，并用变换 containing block 的 fixed 坐标补偿保持 Affix 可见；不修改用户 Block 源码、页面 hash 或 Ant Design 上游。首次验收发现 Affix 在临界区 `static/fixed` 抖振后，算法改为冻结 flow 几何的 `Flow / Pinned / EndClamp` 有限状态机与 Schmitt Trigger；真实往返序列同一方向只切换一次，shell 高度固定 `94px`，页面 `scrollHeight` 不变。点击 Part 2 的既有 `scrollTop=913`、`targetDelta=0` 和 active 合同保持；证据位于 `tmp/test-governance/issue-1910-native-anchor/` 与 `tmp/test-governance/issue-1910-native-anchor-jitter/`。
- 只修改 1flowbase；`/home/taichuy/git/react-grid-layout` 仅作 `2.2.3` 参考源码，不修改、fork 或本地链接。仓库内 patch 绑定精确版本并由 preinstall receipt 守住；升级时先移除 patch 运行 #1899 浏览器 contract，官方版本通过即删除 patch，失败才迁移。
- 后续交互采用连续像素 preview 与响应式网格 commit 双态模型；24 列是 Frontstage desktop profile，不是通用布局内核常量。
