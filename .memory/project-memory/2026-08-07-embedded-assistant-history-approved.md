---
memory_type: project
topic: 内置助手历史会话与持久化恢复已批准
summary: 内置助手历史 contract 已扩展为客户端断连后后端运行继续、历史可切换并 attach、同一 conversation 单 active run；客户端工具断连后返回 client_unavailable 且无页面副作用。
keywords:
  - embedded assistant
  - conversation history
  - conversation_id
  - Conversations
  - issue 1608
match_when:
  - 规划或实现内置助手历史聊天
  - 处理助手会话恢复、分页、旧运行或会话权限
  - 开始 GitHub issue 1608
created_at: 2026-08-07 11
updated_at: 2026-08-15 22
last_verified_at: 2026-08-15 22
decision_policy: verify_before_decision
status: implemented_pending_user_acceptance
integration_commit: f9b9e76db
route_fix_commit: 07706dbae
scope:
  - https://github.com/taichuy/1flowbase/issues/1608
  - web/app/src/features/agent-flow/components/embedded-assistant
  - web/app/src/features/agent-flow/hooks/useEmbeddedAssistantSession.ts
  - web/packages/api-client/src/console-assistant.ts
  - api/apps/api-server/src/routes/assistant.rs
  - api/crates/control-plane/src/application_public_api/run_service.rs
  - api/crates/storage-durable/postgres/src/orchestration_runtime_repository
---

# Embedded Assistant History Approved

## 谁在做什么

后续新会话以线上 Single Issue `#1608` 为唯一计划真值，为内置助手实现历史列表、新建、切换、持久化恢复与继续会话。

## 为什么这样做

当前助手消息只存在前端 session state；虽然已有 durable run、运行摘要与消息投影，但没有稳定内部会话身份。同一用户在同一应用的多段聊天不能仅由应用、用户和协议可靠区分。

## 已批准决策

- 采用平衡方向：复用既有运行日志与消息投影，新增后端拥有的稳定内部会话标识和专用助手历史 contract。
- 查询由后端强制限定当前 session 用户、workspace、application、`assistant_execution` 与 `embedded_assistant`；前端不读取全量日志后过滤。
- 模型供应商 `protocol` 不是会话身份；稳定 `conversation_id` 才标识一段聊天。
- 新 UI 使用 Ant Design X `Conversations` 承担列表、日期分组、选择与新建；消息区继续复用现有 Agent Flow Debug Console。
- 旧助手运行按单次只读快照展示，不按时间、模型、协议或内容相似度猜测合并；可显式基于旧快照新建会话。
- 第一版不做删除、重命名、搜索、置顶、归档或保留策略，不覆盖或重写既有用户内容。
- 如实现需要修改 public conversation contract、不可逆迁移旧内容或建立第二消息 ledger，必须停止并重新对齐。

## 为什么选择该方向

它保留已有投影价值，同时把会话身份、权限、分页和历史语义集中在后端 owner；相比把每个 run 当会话，它能可靠继续聊天，相比完整会话管理系统又避免提前引入删除、迁移和保留策略风险。

## 截止日期

无固定日期；新开发会话读取并执行 GitHub `#1608`，完成 AC、fresh QA 与用户验收后关闭。

## 2026-08-15 长任务与历史续接

- 用户确认关闭助手窗口、刷新或关闭浏览器只断开观察和客户端工具能力，不取消已接受的后端运行；显式 Stop 是唯一取消入口。
- 历史会话在其他 conversation 运行期间仍可选择；选中 active conversation 时读取后端 `latest_flow_run_status` 并 attach `latest_flow_run_id`。
- 同一 conversation 只允许一个 active run，不同 conversation 可各自在后端运行；PostgreSQL partial unique index 是并发兜底。
- 浏览器注入工具只属于创建运行时的当前 WebSocket capability；断连后不执行页面副作用、不自动补执行，并立即返回 `client_unavailable` 工具错误供 Agent 继续处理。
- 本轮不承诺 api-server 进程重启后的任务恢复；该能力仍属于独立 durable scheduler 范围。
- `f9b9e76db` 已 fast-forward 到 `dev`，集中 Dev Acceptance 通过；等待用户人工验收真实窗口关闭、历史切换和长任务完成路径。

## 2026-08-15 历史消息路由修复

- Axum 0.7 运行时动态路径必须使用 `:conversation_id` / `:flow_run_id`；Utoipa OpenAPI 文档继续使用 `{conversation_id}` / `{flow_run_id}`。
- `07706dbae` 修复 conversation 与 legacy snapshot 消息路由，并增加完整认证 Router 回归测试。
- API 重启后，用户报告的 conversation messages URL 已返回 `200` 和两条完整历史消息；功能仍等待用户界面验收。
