---
memory_type: project
topic: Frontstage TSX 代码区块作者平台长期 contract
summary: 用户确认使用原生 BlockModule + main 替换 defineBlock + render，并以 Callable OpenAPI 光标源码生成、类型化 outputs/inputs、Draft Run Console 和非模态 Window Workspace 作为上线前长期架构；计划真值为 Root #1393 及三个 Delivery。
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
updated_at: 2026-07-20 23
last_verified_at: 2026-07-20 23
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
- Page Document 拥有区块 id/title、接口绑定和 Port Schema；源码不建立第二真值。
- 连接器严格按 Callable OpenAPI 在当前光标插入 `$ref` 实体、参数、响应与完整命名函数，不自动改写 `main`，不发明 OpenAPI 不存在的类型。
- `application_conversations` 只暴露真实允许的 list/get；当前 OpenAPI `filter` 为 string 时只生成 `filter?: string`。
- 跨区块共享使用 `BlockResult.outputs → Tab/Page Scoped Signal → ctx.inputs`；Signal 单写多读且 V1 拒绝环，Event Bus 不承担状态。
- 草稿运行统一为单一 `run_id`，观测 Preview、Console、Variables、Interface Calls 与 Problems；写 operation 每次运行单次授权。
- TSX Studio 使用非模态 Window Workspace；桌面可拖动缩放，移动端视口内最大化，主从关闭与脏源码保护由窗口 owner 状态机负责。

## 相邻边界

- #1382 仍独立拥有浏览器 Worker 调度、预算和加载体验；#1393 不改变其 Root AC。
- #1376 仍独立拥有页面网格布局与碰撞语义。
- #1297 是真 JSX、手工 capability 和 Monaco 前言的历史基础，不是当前活动计划真值。

## 执行状态

- Root 已实现并装配到隔离分支 `codex/issue-1393-assembly@6a282258c`；主工作树保持 `beta`。
- 用户明确旧断言、旧测试和无效 fixture 不构成兼容要求；符合已批准新 contract 的 fixture 修正无需重复产品授权。
- canonical host integration fixture 已在 `56e19a4a4` 补齐 tabs presentation、route segment、每个 Tab 的 document root 与 `renderer_version=v1`；目标测试 3/3 通过，AC-004/AC-009 已结算。
- Assembly 已通过 `--ff-only` 整合到本地 `beta@56e19a4a4`，未生成额外 merge commit，远端尚未 push；#1393 assembly 与 D1 worktree/branch 已安全回收。
- Root 进入 `phase:user-acceptance`；用户负责 AC-008/AC-010 的最终浏览器验收，通过后再 push beta 并关闭 Issue Tree。
