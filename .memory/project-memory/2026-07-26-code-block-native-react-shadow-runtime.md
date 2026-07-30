---
memory_type: project
topic: 代码区块 Native React + Shadow DOM 运行时方向
summary: Native React + Shadow DOM、R4 与 R5 Studio/Catalog 均已合入 beta；R5 通过 compiler-owned fixture contract reframe、集中 QA 与 desktop/mobile 验收，Root #1466 等待用户人工验收。
keywords:
  - code block
  - native React
  - Shadow DOM
  - raw CSS
  - compile worker
  - component catalog
  - runtime state machine
match_when:
  - 规划或实现代码区块 Native React runtime
  - 修改 Worker 执行边界、渐进加载、artifact cache 或运行状态机
  - 调整 Frontstage/Auth 共享代码区块 host
created_at: 2026-07-26 22
updated_at: 2026-07-27 22
last_verified_at: 2026-07-27 22
decision_policy: verify_before_decision
status: user_acceptance
source_issue: "#1466"
delivery_issues:
  - "#1469"
  - "#1467"
  - "#1468"
  - "#1470"
  - "#1475"
  - "#1476"
supersedes_decisions:
  - 代码区块 UI 必须返回 BlockUiSchema
  - Web Worker 必须执行 BlockModule.main(ctx)
scope:
  - web/packages/page-runtime
  - web/packages/page-protocol
  - web/packages/block-renderer
  - web/packages/antd-facade
  - web/app/src/features/frontstage
  - web/app/src/features/auth
  - api/crates/plugin-framework
  - api/crates/domain
  - api/crates/control-plane
---

# 代码区块 Native React + Shadow DOM 运行时

## 谁在做什么

1flowbase 正在为现有代码区块重新对齐长期运行时：作者直接编写标准 React Component，Host 将编译后的组件挂载到区块自己的 ShadowRoot；Frontstage、认证中心 Studio 和公开认证页面继续共享同一代码区块 runtime/host。

## 为什么这样做

现有 `main(ctx) -> BlockResult.view -> BlockUiSchema -> facade renderer` 要求作者和通用聊天模型理解私有组件与样式协议，普通 `style={{ textAlign: 'center' }}` 会被静默过滤。Shadow DOM 把样式隔离复杂度收回 runtime owner，同时保留正常 React、DOM、Hooks、事件和 CSS 作者体验。

## 为什么要做

目标是让标准 React/CSS 知识直接有效，并让区块内样式无法污染宿主或相邻区块；UI 设计模式权限负责作者和发布授权，不通过 CSS 属性白名单表达权限。

## 截止日期

未指定。新的两层 Issue Tree 已上线并进入 `phase:ready`。

## 已确认决策

- 使用 Native React + Shadow DOM，不使用嵌套 iframe。
- 继续使用受控依赖 Catalog，但 CSS 不做属性或选择器白名单。
- 新架构不为旧 `BlockUiSchema`、`antd-facade` 或 `BlockModule.main(ctx)` UI 路径保留兼容分支。
- 现有浏览器渐进加载、缓存、Signal 与状态机必须按新生命周期重新判断，不能机械照搬或全部推倒。
- React Component 在主线程运行；正常生命周期、CSS 与普通 render error 必须区块级隔离，但不承诺同步死循环的 Worker 级硬终止。

## 当前线上结果

- Root：#1466，`grade:g4`、`hybrid-foundation`、`phase:user-acceptance`。
- D1 #1469：标准 React 组件在共享 Studio 中隔离编译预览。
- D2 #1467：受控 React 组件 Catalog、按需模块加载与 Artifact V2。
- D3 #1468：Frontstage 原生区块渐进挂载、响应式 Signal 与生命周期。
- D4 #1470：认证 UI 切换共享 Native Host 并移除旧 facade 执行链。
- #1469、#1467、#1468、#1470 已在 centralized QA 通过后关闭。
- verified assembly：`26c8576e2a783ca63ef5610585c743eb0ee2831d`；已合入并推送 `beta@5931db9f616e9ef3c34aa20ca6e574207535e59b`。
- 旧 Root #1382 与旧 Catalog #1459 已写 superseded 评论并以 `phase:closed` 冻结；旧 Delivery 保留证据、不重开。

## 2026-07-27 Portal reframe 与 QA 结果

- 每个应用表面使用自己的 React Root；区块不再 `createRoot()`，而由 surface tree `createPortal()` 到独立 ShadowRoot。
- PageCanvas 只负责 layout/demand；PageSignalStore 用 `subscribeBlock/getBlockSnapshot + useSyncExternalStore` 精确通知 DAG 下游，不再中转 revision 或使用延迟补丁。
- Compiler Worker、完整 Artifact V2 identity、IndexedDB byte-LRU、Module Registry、dependency lock、epoch/stale rejection、Auth canonical session 与 legacy fail-visible/no-rewrite 均保留。
- QA：page-runtime 13/124、app targeted 15/82、scheduler 2/13、web build、Rust Catalog 26、desktop/390 browser 全部通过。
- 浏览器证据覆盖 CSS/variables/AntD popup 隔离、publish event completion、PageCanvas render 不变、Hooks 保留、source exact remount/new epoch、render fallback、page exit cleanup、Public Auth native/legacy。

## 2026-07-27 R4 公共模块扩展状态

- 最终 assembly `ae3d77156e2d5604b862584790cb816641e67983` 已通过公共 package、App targeted 84、api-client 173、确定性资产、Rust Catalog 30、PostgreSQL atomic 1、web production build 与 desktop/390 browser。
- 匿名 404 已归因为 fixture 未声明 favicon；使用内联 favicon 后保持零 HTTP/failed/external request 与零 console error。
- mobile PageCanvas 计数误报已归因为 fixture 直接调用 `PageCanvas(props)` 合并子组件 hooks；恢复 `<PageCanvas {...props} />` React 边界后 desktop/390 均为 `6 → 6`。
- assembly 已 fast-forward 合入并推送 `beta@ae3d77156`；#1472/#1473 已关闭，Root #1466 已切到 `phase:user-acceptance`。

## 2026-07-27 R5 Studio/Catalog 状态

- R5 产品 assembly `36b3506f6011ef82f43b75bcb0272a1a7c72b71c` 已完成 Catalog-aware Monaco、Preview Console、统一 Add Block Drawer 与前端模板真值清理。
- QA cycle 1 的产品真实性证据为绿；失败集中在测试 fixture contract、React 19 Ant Design patch、hidden telemetry wait 与隔离 worktree 依赖/重型 Cargo 入口。有限 fix `07d770ea499bb10b6e94a800c55d577bce6c6453` 已装配。
- QA cycle 2 再次发现同类 fixture expectation debt：旧按钮文案“插入代码”与真实“插入”不一致，组件搜索仍断言 `textbox` 而真实角色为 `searchbox`；inventory ENOENT 是 QA cwd 错误。
- 用户随后批准完整 fixture inventory reframe；assembly `3b174ebfaba31a394d787d7c06c0754beda1b076` 从 clean 状态启动 fresh centralized QA。
- fresh QA 已通过 page-runtime 58/58、App targeted 65/65、api-client 30/30，以及 api-server/control-plane/storage-postgres 三条串行 `--lib` 定向测试；App TypeScript 随后发现 `JsxStudioResourcePanel.test.tsx` 把 `allowedImportSources` fixture 写成数组，而 contract 要求 `ReadonlySet<string>`。
- 该结果仍是 reframe 试图一次性收口的 fixture inventory 同根因，因此再次触发 Root hard stop；Vite/i18n/style-boundary/browser/source inventory 未继续，未合入或推送 beta。#1466 当前为 `phase:discussion`，#1475/#1476 保持开放。
- 用户授权在没有新产品决策时持续执行；最终 reframe 将 TypeScript 编译器设为 fixture contract 完整判定器，而不再依赖文本 inventory。唯一手写 projection fixture 改为 `new Set<string>()`，commit 为 `b14251b2a2d129e0a87ce4dd11b8e17fd149562d`。
- fresh centralized QA 最终通过：page-runtime 58/58、App 65/65、api-client 30/30、Rust 1/2/1、App TypeScript、Vite build、i18n、14 个 style boundary、source inventory，以及 desktop/mobile 正式浏览器 runner；浏览器保持零 external/HTTP/failed/console error。
- assembly 已 fast-forward 合入并推送 `beta@b14251b2a2d129e0a87ce4dd11b8e17fd149562d`；#1475/#1476 已关闭，#1466 进入 `phase:user-acceptance`。本任务 assembly clone 已清理，最终 QA evidence 保留在主工作树 `tmp/test-governance/root-1466-r5-final-rerun/`。
