---
memory_type: project
topic: Frontstage TSX 代码区块作者平台长期 contract
summary: 用户确认使用原生 BlockModule + main 替换 defineBlock + render，并以 MCP/Studio 共享 OpenAPI Capability Catalog、光标源码生成、类型化 outputs/inputs、Draft Run Console 和非模态 Window Workspace 作为上线前长期架构；Root #1393 已合并本地 beta 等待浏览器验收。
keywords:
  - frontstage
  - tsx studio
  - block module
  - openapi codegen
  - callable operation
  - block outputs
  - scoped signal
  - draft run
  - window workspace
match_when:
  - 规划或实现 Frontstage TSX 编辑器、接口连接器、代码区块 SDK 或运行控制台
  - 调整区块输入输出、跨区块联动、Page/Tab Signal 或 Event Bus
  - 评估 defineBlock/render、OpenAPI 生成源码或 Studio 浮窗交互
created_at: 2026-07-20 19
updated_at: 2026-07-21 21
last_verified_at: 2026-07-21 21
decision_policy: verify_before_decision
status: local_beta_user_acceptance
source_issue: "#1393"
delivery_issues:
  - "#1396"
  - "#1395"
  - "#1394"
scope:
  - api/apps/api-server/src/runtime_data_model_docs.rs
  - api/apps/api-server/src/routes/frontstage
  - web/packages/page-protocol
  - web/packages/page-runtime
  - web/app/src/features/frontstage/components/jsx-studio
  - web/app/src/features/frontstage/lib/jsx-studio
---

# Frontstage TSX 代码区块作者平台长期 contract

- 谁在做什么：Frontstage 在上线前建立新的代码区块作者 contract；Root #1393 是唯一计划与用户验收真值，#1396、#1395、#1394 分别交付 OpenAPI 源码与安全执行、类型化区块联动、草稿运行与浮动 Studio。
- 为什么这样做：当前接口连接器、Monaco 前言、试运行面板与 Drawer 是链路验证实现，无法长期承载通用 OpenAPI、结构化数据流、可观测草稿运行和多窗口作者体验。
- 为什么要做：若上线后再改变模块入口、绑定格式、Page Document 端口和运行结果 contract，会引入用户源码兼容与持久化迁移成本；当前可在上线前干净切换。
- 截止日期：未指定；要求在用户上线前冻结并交付。
- 决策动机：后端保持 OpenAPI、DTO、权限和运行状态唯一真值；用户源码保持完整、可见、可编辑，宿主吸收认证、作用域、权限、序列化和调度复杂度。

## 已确认决策

- 新源码使用 `async function main(ctx): Promise<BlockResult>` 与 `export default { main } satisfies BlockModule`；不再使用 `defineBlock({ render })`。
- Page Document 拥有区块 id/title 和 Port Schema，不再保存或 round-trip `interfaces`；源码是作者调用真值，Capability Catalog 是接口 Schema 真值。
- MCP 与 Studio 共同消费后端唯一的 OpenAPI Capability Catalog；共享静态 Console OpenAPI、动态数据模型完整 CRUD、参数/返回 Schema、风险和 bindable 真值，不再维护 Frontstage 私有目录或 MCP 私有 OpenAPI 解析器。
- Studio 目录使用 `GET /api/console/frontstage/{workspace_id}/interface-capabilities` 与 `frontstage.page.design` 设计权限；运行 dispatch 继续由页面访问者的目标 API 权限决定。Published Workflow / Agent Flow 不进入 Studio 目录。
- 连接器严格按 Capability Catalog 在当前光标插入 `$ref` 实体、参数、响应与完整命名函数，不自动改写 `main`，不发明 OpenAPI 不存在的类型。接口选择器只显示 `METHOD /path`，用户只搜索 path，来源、Method 与分页由后端执行。
- 选择接口后只加载详情、生成一个参数/返回结构内联的 callable variable 并插入光标；不保存 binding，不展示已绑定/取消绑定，也不调用 `onSaveBlock`。作者与 Runtime 使用受控 `ctx.api.<method>(path, request)`；HTTP 身份是规范化 method + path template，不暴露 `interfaceId`、`schemaDigest` 或 descriptor。
- OpenAPI DTO 保持后端字段原名；与 JS Block 静态策略冲突的属性名（如 `document`）生成字符串属性键，不能让合法响应字段触发全局标识符诊断。
- 跨区块共享使用 `BlockResult.outputs → Tab/Page Scoped Signal → ctx.inputs`；Signal 单写多读且 V1 拒绝环，Event Bus 不承担状态。
- 草稿运行统一为单一 `run_id`，观测 Preview、Console、Variables、Interface Calls 与 Problems；写 operation 每次运行单次授权。
- UI 设计模式画布只展示区块最终视觉结果和设计控件；日志、效果、拒绝项及内部错误详情不进入画布，运行诊断统一留在 TSX Studio 的 Console、Variables、Interface Calls 与 Problems。
- TSX Studio 使用非模态 Window Workspace；桌面可拖动缩放，移动端视口内最大化，主从关闭与脏源码保护由窗口 owner 状态机负责。
- TSX Studio 的作者工作区采用右侧资源布局：桌面为 `Editor | Resource Panel | Rail`，代码单栏为 `Editor | Rail`；移动端资源面板与编辑器在左列纵向堆叠，Rail 固定最右侧。
- 自动生成的 `@1flowbase-context` 注释由标题栏“注入上下文”动作写回源码；按钮不占用 Monaco 内容区，删除注释后仍可重新注入。
- Monaco 只负责 TSX 解析、补全与类型检查，使用 `JsxEmit.Preserve`；Page Runtime 是 JSX 变换唯一 owner，继续用受控 `antd-facade` 的 `h / Fragment` 编译，不引入或伪造 `react/jsx-runtime`。
- Window Workspace 首次打开、最大化和浏览器缩放必须完整适配顶部导航栏以下的可用视口并保留 8px 边距；自由拖动仍允许窗口重叠，但标题栏必须保持可找回。窗口内容不足时在窗口内部滚动，不能让窗口初始底部沉出视口。
- TSX Studio 桌面首次打开使用 `1080 × 680` 默认尺寸，顶部保持在导航栏下方；不默认铺满可用高度。移动端继续在可用视口内自动最大化，桌面最大化和手动缩放能力不变。

## 相邻边界

- #1382 仍独立拥有浏览器 Worker 调度、预算和加载体验；#1393 不改变其 Root AC。
- #1376 仍独立拥有页面网格布局与碰撞语义。
- #1297 是真 JSX、手工 capability 和 Monaco 前言的历史基础，不是当前活动计划真值。

## 执行状态

- 第一版 descriptor assembly `fa6bbeccf` 已被用户否决并被后续实现取代；最终 RouteKey 重构提交为 `77fa65a4d`，已合并到本地 `beta@6a4f92632`。
- 用户明确旧断言、旧测试和无效 fixture 不构成兼容要求；符合已批准新 contract 的 fixture 修正无需重复产品授权。
- canonical host integration fixture 已在 `56e19a4a4` 补齐 tabs presentation、route segment、每个 Tab 的 document root 与 `renderer_version=v1`；目标测试 3/3 通过，AC-004/AC-009 已结算。
- 自动化证据：page-runtime 215/215、api-client 164/164、Frontstage/Host 定向 71/71、api-server callable route 6/6、Catalog 模板真实性 1/1、page-protocol 24/24、block-sdk 8/8 通过；全 App TypeScript 仍只有 beta 既有的 applications/agent-flow 无关基线错误。
- Root 进入 `phase:user-acceptance`；用户负责最终浏览器验收，通过后再 push beta 并关闭 Issue Tree。远端尚未 push。
