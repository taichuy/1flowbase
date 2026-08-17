---
memory_type: project
topic: 内置助手历史会话与持久化恢复已批准
summary: 内置助手历史 contract 已扩展为客户端断连后后端运行继续、历史可切换并 attach、同一 conversation 单 active run；显式 Stop 会真实终止执行；每次 flow_run 由 Run Event Ledger 持久化并按 sequence 确定性恢复完整活动，终态不再把中间正文整体收进外层折叠；左侧运行栏保留完整节点卡片。
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
updated_at: 2026-08-16 23
last_verified_at: 2026-08-16 08
decision_policy: verify_before_decision
status: implemented_pending_user_acceptance
integration_commit: d845e4a61
route_fix_commit: 07706dbae
activity_timeline_commit: e39e1e2b1
activity_presentation_commit: 3a28140a8
ordered_activity_commit: f51fa3b4d
canonical_history_commit: e57173ee1
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

## 2026-08-16 显式 Stop 与取消终态

- 显式 Stop 是运行取消的唯一用户入口；它必须中止真实 detached Assistant execution，不能只把 `flow_runs.status` 写成 `cancelled` 后继续调用模型或工具。
- 取消前已形成的 Answer Presentation 公开文本作为 partial answer 持久化；provider reasoning 不进入最终回答或长期会话内容。没有公开文本时才展示“运行已取消，未产生输出”。
- 未完成节点统一进入 `cancelled`；取消前已完成的 tool node 与 trace 保留可查。Flow、Node、runtime event、应用日志和 Assistant history 使用同一 durable terminal。
- terminal commit 后，ordinary flow/runtime event、迟到 node create/complete 都必须被 repository fence 拒绝；canonical terminal events 仍由同一事务写入。
- Assistant history message DTO 继续使用领域原名 `content + status`，不新增展示别名 `answer`；live、reconnect、history 与 run detail 消费同一合同。
- 合并提交 `d845e4a61` 已进入本地 `dev`；集中后端 34/34、前端 50/50、api-client 定向 12/12（额外整包 233/233）与 style-boundary 均通过，等待用户人工验收真实 Stop 与历史恢复路径。

## 2026-08-16 单次运行活动分离（用户纠正后）

- 用户明确纠正此前理解：主聊天区不能只显示最终公开回答，必须按 durable `sequence` 展示 reasoning、工具调用/结果、阶段输出，再继续 reasoning、工具和输出的真实交错顺序。
- 消息及标题栏“运行过程”入口仍打开单次 `flow_run` 侧栏；侧栏恢复修改前的完整工作流与全部节点卡片，只是不在节点卡片内显示 reasoning。不得把侧栏降级为单个当前/最后节点摘要。
- 后端新增 assistant-owned 只读活动查询，按 durable stream `sequence` 返回事件，并强制限定当前 Cookie 用户、workspace、application、`assistant_execution` 与 `embedded_assistant`；外部运行返回 404。
- 实时事件与历史恢复复用同一事件模型形成两个投影：主聊天是有序行为叙事；侧栏是完整节点状态与详情。通用 Agent Flow Debug Console 保持原行为，仅内置助手的 answer presentation 使用新主内容投影。
- 桌面仍使用聊天 + 运行过程双栏；窄屏运行过程独占助手窗口。修正实现完成定向 42/42、生产构建与 style-boundary，等待合并后用户人工验收。

## 2026-08-16 双栏边缘缩放语义（用户二次纠正后）

- 桌面结构固定为“左外边缘 → 运行过程侧栏 → 中间分隔线 → 主对话 → 右外边缘”；不得为了调整缩放语义而把运行侧栏移到右侧。
- 水平拖拽采用 nearest pane owner：左外边缘只增减左侧运行栏，右外边缘只增减右侧主对话，中间分隔线在外层窗口宽度不变时反向分配两栏宽度。
- 数学不变量：左边缘 `Δsidebar = ΔwindowWidth, Δconversation = 0`；中间 `Δsidebar = ΔdividerX, Δconversation = -ΔdividerX`；右边缘 `Δsidebar = 0, Δconversation = ΔwindowWidth`。

## 2026-08-16 助手尺寸浏览器缓存边界

- 用户确认浏览器 Storage 只保存主对话 pane 宽度 `conversationWidth` 和助手窗口高度 `windowHeight`。
- 不保存窗口位置、运行栏宽度、侧栏开关、最大化状态或外层双栏总宽。侧栏打开时 `conversationWidth = outerWidth - sidebarWidth - dividerWidth`。
- Storage 只在拖拽交互结束时写入，恢复时按当前 viewport 夹取；左外边缘只改运行栏时不改变缓存的主对话宽度。

## 2026-08-16 活动事件终态收纳与工具摘要

- 运行中按 durable `sequence` 展示 reasoning、工具调用/结果和阶段输出；所有节点事件进入终态后，把最终输出之前的全部过程收进单一“耗时”折叠组，最后一段输出始终在外部可见。
- 思考不再使用 `ThoughtChain item -> Think` 双层折叠；运行中只有一层思考折叠，终态展开过程组时不再为思考叠加同名容器。
- 工具默认行使用“工具名（主要定位值）”：`path` 显示路径值，`group_id` 显示 group id 值，不展示 JSON 键名；展开后再显示完整 Input / Output。
- `3a28140a8` 已 fast-forward 合入本地 `dev`；后端定向 6/6、API client 235/235、嵌入助手 25/25、16-package 生产构建、Rust 静态门禁与样式边界通过，等待用户人工验收真实运行时序和终态折叠。

## 2026-08-16 Run Event Ledger 时序恢复修正

- 用户确认以成熟 ordered event log 机制为准：LLM Runtime 产生规范化语义事件，Run Event Ledger 是 run-level 全局序号与持久化唯一 owner；前端只按 `sequence_start/sequence_end` 做确定性投影。
- `text_delta/reasoning_delta` 恢复 `persist_required=true`，历史 persister 继续按相同 projection identity 压缩连续区间；工具与生命周期事件是不可跨越的 barrier。
- Assistant activity 只消费 Answer Presentation reasoning/output；节点调试副本继续留在 trace，避免同一思考重复展示。
- 此决策取代本文件上一节“终态把最终输出之前全部过程收进单一折叠组”的交互：完成态保留中间正文、思考与工具的原始相对顺序，只把最后正式输出单独放在末尾。
- 工具摘要补齐 `list(path) / get(tool_id) / call(tool_id)`；旧运行只能改善标题，已丢失的中间流序号不做猜测性回填。
- `f51fa3b4d` 已 fast-forward 合入本地 `dev`；Rust activity 5/5、嵌入助手 29/29 和主工作树关键 3/3 通过，等待用户人工验收真实运行、完成与刷新恢复三态一致性。

## 2026-08-16 Canonical thinking history 与确定性错误重试

- 内置助手续轮历史改为由后端根据 `conversation_id` 从 durable projection 重建；前端不再把包含 `<think>` 的展示消息反向提交为模型历史。
- succeeded LLM 的 canonical assistant message 持久化 `content_blocks` 与 `tool_calls`；reasoning block 由 DeepSeek Provider 映射回 `reasoning_content`。
- Provider typed upstream error 同时支持 canonical `status_code` 与旧 `status`；HTTP 400 不再消耗 LLM 节点重试预算，DSML output protocol failure 继续复用节点纠错重试。
- 核心 `e57173ee1` 已推送 `dev`；DeepSeek `0.1.23` 已发布六平台包并写回 official registry。自动化通过，等待用户人工验证同一 thinking conversation 的第二轮请求与 DSML 异常重试路径。
