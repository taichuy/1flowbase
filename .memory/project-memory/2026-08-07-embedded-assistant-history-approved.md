---
memory_type: project
topic: 内置助手历史会话与持久化恢复已批准
summary: 用户已批准为内置助手增加稳定内部会话身份和专用历史接口；复用运行日志与消息投影，旧运行只读且不猜测合并，线上实施真值为 GitHub #1608。
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
updated_at: 2026-08-07 11
last_verified_at: 2026-08-07 11
decision_policy: verify_before_decision
status: approved
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
