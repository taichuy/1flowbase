# Frontend Review Checklist

## Before

- 这次改动属于哪个任务域？页面 recipe 有没有越界？
- 这次改动是不是新页面、新流程、交互流或视觉方案？
- 如果本轮涉及入口、层级、详情容器或执行落点，是否已先运行 `interaction-architecture-gate.md`？
- 如果当前属于页面 / UI 开发需求，是否已先在回复里输出需求整理 / 需求细化？
- 如果用户输入是模糊目标、图片或外部样本，是否已先产出设计需求草案再进入实现？
- 先从第一体验用户视角走一遍，确认是否真的顺手、直观
- 是否已有现成组件、成熟依赖、可复用模式？
- 当前改动会不会引入新的页面语法、L1 模型或新的交互规则？
- 首屏主任务、L1 详情模型、L2 管理入口、L3 执行动作和反馈落点，是否都能一句话说清？
- 状态应该放在页面、局部组件、共享状态还是协议层？
- 视觉判断是不是仍然服从 `DESIGN.md`？
- 这次样式改动属于 `theme token / first-party wrapper / explicit slot` 哪一层？
- 如果碰第三方内部布局，blast radius 和验证证据是什么？
- 导航文案、`route id`、`path`、选中态是否仍来自同一真值层？

## During

- 当前文件是否同时承载了多个变化原因？
- 是否为了“以后可能复用”提前抽组件或 hooks？
- 是否把异步状态、表单状态、展示状态、弹窗状态全堆在一起？
- 是否把协议细节、UI 展示和业务判断写进同一个位置？
- 是否出现了裸 `.ant-*` 选择器或跨多个第三方内部节点的递归后代选择器？
- 是否修改了第三方内部布局指标：`display / position / height / line-height / padding / gap / overflow`？
- 是否在 Shell / Canvas 间混用了 `Drawer` 和 `Inspector`？
- 是否把状态色拿去做类型色、装饰色或品牌色？

## After

- 同类组件行为是否一致？
- L0 / L1 / L2 / L3 是否仍然清楚，且没有把重操作塞回概览层？
- 第一体验用户从入口走到完成是否仍然自然？
- 新实现是否仍然服从既有页面 recipe？
- 风格和 UI 质量是否已经被当成本轮验收项验证，而不是主观假设？
- 第三方原生组件的交互、布局和图标链路是否仍然成立？
- 当前样式改动是否能说清 blast radius，且已检查受影响消费者？
- 本次改动是否已经运行 `node scripts/node/tooling.js check-style-boundary component ... / page ... / file ...` 中至少一种合适模式？
- 如果需要浏览器级验收、截图或交互复现，是否已默认使用 `Playwright`，而不是 Chrome 浏览器 MCP / `chrome-devtools`？
- 如果当前只知道页面路由、需要自动登录、稳定等待或导出运行态证据，是否优先运行 `node scripts/node/page-debug.js snapshot|open ...`，而不是临时手写一次性 Playwright 脚本？
- 浏览器级等待、截图和操作是否基于业务 ready signal，而不是页面一打开就直接执行？
- 页面存在规范化跳转时，是否已用 `--wait-for-url <final-url>` 对齐当前运行态，而不是沿用旧路由假设？
- 若已运行 `page-debug`，是否检查了 `outputDir` 下的 `meta.json / page.png / console.ndjson`，必要时再看 `index.html / css / js`，而不是只看单张截图？
- 如果改动影响共享样式或第三方 slot，`web/app/src/style-boundary/scenario-manifest.json` 是否已经补上对应的页面/组件场景与 `impactFiles` 映射？
- `boundaryNodes / propertyAssertions` 是否只表达样式边界断言，而没有混入泛视觉主观描述？
- 若出现“样式边界失败 / 样式扩散失败”，失败截图和样式来源证据是否已进入 `uploads/`，而不是只给口头判断？
- 如果本轮最初是参考图驱动，是否已经明确说明“借什么，不借什么”，而不是把第三方视觉当成当前产品规范？
- 如果本轮属于页面 / UI 开发需求，回复里是否已经把任务理解、改动范围、关键状态和明确建议显式发给用户？
- 如果本轮命中过交互架构 gate，回复里是否已经说明首屏主任务、L1 / L2 / L3 和反馈落点？
- 导航文案、`route id`、`path`、选中态是否仍一致？
- L1 详情模型是否仍然只剩 `Drawer` 或 `Inspector` 两种？
- 列表状态点、节点 badge、Inspector 状态字段是否仍然一致？
- 是否引入了新的隐藏状态或难以追踪的局部逻辑？
- 后续需求是否会被迫继续修改同一大片代码？
- 小屏是否做了诚实降级，而不是把桌面画布硬塞进移动端？
- 是否留下了 no-op 按钮或内部指令式文案？
