---
memory_type: project
topic: MCP 创建 Workflow 并发布 API 后缺少安全的公开 operation 转 Tool 边界
summary: 用户已批准 Workflow 长期架构方案与 run_mode CHECK migration；P01～P05 已装配并在 f2302519b 完成首次集中 QA。QA 发现 Schedule 幂等仍缺 PostgreSQL partial unique index，且 enqueue 失败无法恢复，当前再次等待用户确认最小数据约束扩展。
keywords:
  - MCP
  - workflow
  - application public API
  - interface catalog
  - unsupported_mcp_interface_scope
  - application_operation
  - mcp-workflow-echo-20260720
  - issue 1387
match_when:
  - 继续把已发布 Workflow API 转换为 MCP Tool
  - 设计应用公开 API 进入 MCP interface catalog 的认证与所有权
created_at: 2026-07-20 12
updated_at: 2026-07-20 18
last_verified_at: 2026-07-20 18
decision_policy: verify_before_decision
scope:
  - application 019f7def-ef16-7610-a553-dfd87fe1e8ed
  - /api/ex/mcp-workflow-echo-20260720
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - api/crates/control-plane/src/application_public_api
  - https://github.com/taichuy/1flowbase/issues/1387
---

# MCP Workflow Echo Public API Gap

## 当前状态

- 已通过 MCP 创建 Workflow 应用 `MCP Workflow Echo`，application id 为 `019f7def-ef16-7610-a553-dfd87fe1e8ed`。
- Workflow 使用 `workflow_start -> template_transform -> workflow_end`，必填 body 参数 `message` 原样返回。
- 草稿已保存并发布；publication id 为 `019f7df1-3cbe-7352-bd16-3a5965cf703e`，公开 URL 为 `/api/ex/mcp-workflow-echo-20260720`，`active=true`、`api_enabled=true`。
- 已通过 MCP 创建并挂载 Workflow 创建、编排读取/保存、API mapping、发布和 API docs 查询 Tool。

## 阻塞证据与纠正

- 纠正旧结论：发布后的 `/api/ex/{slug}` 已由 `dynamic_openapi` 注册进全局 `/openapi.json`，不是“没有注册接口”。
- MCP interface catalog 只装配 static API docs 与 runtime data model CRUD，因此没有消费已经存在于动态 OpenAPI 的 `/api/ex/{slug}` operation。
- 静态 Native 路由 `POST /api/agent/v1/runs` 可被发现，但 `bindable=false`，`disabled_reason=unsupported_mcp_interface_scope`。
- 当前 `/api/ex/{slug}` route 和 service 仍强制解析并认证 Application API Key，并校验 Key 绑定的 application 与 slug publication 相同；这与用户记忆中的“只有 AgentFlow 才需要每应用 Key”不一致，需要产品决策。
- MCP interface wrapper 会原样转发 MCP User API Key 的 Authorization header；即使补齐动态 catalog，也会被 `/api/ex` 当成错误的 Application API Key 拒绝。
- 已批准的 Workflow contract 要求 HTTP source 直接来自 `workflow_start.config.input_fields`、删除 target selector；当前运行链仍从 `publication.mapping_snapshot.extension.parameters` 读取 `target` 并写 selector。仓库已有 `workflow_start_http_inputs` 解析器且明确拒绝 target，但尚未接入 `WorkflowExtensionRunService`。在转 MCP Tool 前应先统一这份参数真值，不能继续固化第二套 mapping。

## 已批准架构边界（2026-07-20 16）

- 用户明确把 Workflow 视为仍在开发中的新产品能力，当前覆盖 `/api/ex/*` 扩展接口触发与定时触发。
- 用户判断初期实现从 AgentFlow 复制过多，已经造成产品 contract、认证、发布和运行语义边界模糊；后续应作为长期架构演进处理，而不是只修 MCP catalog 或 bearer 转发。
- Application 只作为共享壳；AgentFlow 与 Workflow 是两个 bounded product contracts，触发、认证、发布和运行 contract 分别归属各自产品。
- AgentFlow 保留 AgentFlow 专属 Start / End 与 Application API Key；Workflow 保留 Workflow Start / End，并由 Start 输入与 End 输出分别作为唯一真值。
- Workflow 当前一个应用只选择 HTTP Extension 或 Schedule 一个触发器；HTTP route、OpenAPI 与 MCP 共用稳定的 Published Workflow Operation。
- Workflow HTTP 默认使用 User API Key，只有显式 `public` 才允许匿名；不再借用 AgentFlow Application API Key。
- 用户已确认节点注册规则：通用执行节点默认共享，产品专属节点显式绑定 application type，trigger-specific 只作为产品专属节点中的更小例外；不为每个共享节点重复声明产品适用范围。
- 不拆独立 Workflow runtime / run / logs，不兼容尚未正式发布的旧 mapping。

## 在线计划真值

- Root：[#1387 Workflow 产品边界与双触发架构演进](https://github.com/taichuy/1flowbase/issues/1387)，当前因 migration 边界回到 `phase:discussion`，仍是计划、进度与用户验收唯一真值。
- D1：[AgentFlow 产品边界 #1391](https://github.com/taichuy/1flowbase/issues/1391)。
- D2：[Workflow 编辑与节点边界 #1392](https://github.com/taichuy/1flowbase/issues/1392)。
- D3：[HTTP Workflow / OpenAPI / MCP #1390](https://github.com/taichuy/1flowbase/issues/1390)。
- D4：[Schedule Workflow #1388](https://github.com/taichuy/1flowbase/issues/1388)。
- D5：[运行来源、日志与监控 #1389](https://github.com/taichuy/1flowbase/issues/1389)。
- 五个 Delivery 已作为 #1387 的真实 GitHub sub-issues；旧 #1186、#1187、#1188、#1189、#1236 已关闭并保留 superseded 证据回链。

## 当前执行状态（2026-07-20 18）

- P01～P05 与已批准的 run_mode CHECK migration 已装配到隔离 assembly `f2302519bea76277e71b121d8fbba1c7c92718e9`，尚未合入 `dev`。
- CHECK migration 源码只增加 `workflow_http_run`、`workflow_schedule_run`，没有新增列或回填历史数据；尚未连接数据库执行 migration smoke。
- 首次集中 QA 已完成并失败，下一步等待确认 Schedule idempotency partial unique index 后组装单批 fix Packets。
- 新增 Delivery，或改变 Root AC、source of truth、认证、用户内容、migration、contract 时停止执行并回到 `problem-framing`。
- 当前无固定截止日期；完成口径是 Root 全部 AC 有证据、集中 QA 通过并由用户最终验收，而不是单个 Delivery 或局部提交完成。

## 首次集中 QA（2026-07-20 18）

- 用户已确认并完成源码装配的 run_mode CHECK migration；P01～P05 最终冻结 assembly 为 `f2302519bea76277e71b121d8fbba1c7c92718e9`。
- 首次 centralized QA 为 `QA_FAIL`：2 Blocking、2 High、2 Medium。编译导出、archive consumer、Workflow compatibility_mode、错误映射、过期前端期望和 i18n key 复用属于既定范围内 fix。
- 新数据边界：Schedule 当前是 check-then-create；现有 unique index 只保护 `published_api_run + api_key_id`，不能保护 `workflow_schedule_run`。并发 tick 可重复建 run；enqueue 失败后重试命中已有 run 又跳过 enqueue，形成永久半成功。
- 当前最小建议：新增 Schedule idempotency partial unique index；repository 原子 create-or-get；无论 run 是否已存在都使用同一 idempotency key 恢复 queue enqueue。不新增列、不回填历史数据。
- Root / D4 已返回 `phase:discussion`；阻塞证据回写 [#1387 comment](https://github.com/taichuy/1flowbase/issues/1387#issuecomment-5021560975)。
- 仍未连接数据库或执行 SQL/migration；真实 PostgreSQL constraint/migration smoke 需要单独授权。
