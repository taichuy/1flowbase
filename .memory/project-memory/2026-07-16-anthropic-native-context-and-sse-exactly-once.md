---
memory_type: project
topic: Anthropic Native 上下文与兼容 SSE exactly-once 修复
summary: Claude Code/AionUI 续聊历史经 Native prompt context 跨节点进入 Provider Invocation；Answer Presentation 按真实激活分支 exactly-once 投影；兼容 SSE 终态轮询只读取轻量 stream state，不再物化完整运行详情与 stitched trace。
keywords:
  - anthropic
  - ai native
  - claude code
  - aionui
  - conversation history
  - sse
  - exactly once
created_at: 2026-07-16 12
updated_at: 2026-07-17 13
last_verified_at: 2026-07-17 13
decision_policy: verify_before_decision
status: delivered
scope:
  - api/crates/plugin-framework/src/provider_contract.rs
  - api/crates/control-plane/src/application_public_api
  - api/crates/storage-durable/postgres/src/orchestration_runtime_repository
  - api/crates/orchestration-runtime/src/execution_engine
  - api/apps/api-server/src/routes/application_public_api/compat_sse
---

# Anthropic Native 上下文与兼容 SSE exactly-once 修复

## 谁在做什么

AI 按用户要求修复 Claude Code 经 AionUI 调用 1flowbase 时的续聊上下文丢失与回复重复；AionUI 仅作为真实客户端和数据证据，没有修改其源码。

## 为什么这样做

旧 run `019f68f0-c222-7372-ac4d-fbd1fd46a8d2` 已把 2 条历史映射到 `node-start.history`，但 LLM 前有 If/Else，runtime 只检查直接依赖，最终 Provider Invocation 仅有当前问题。旧 run `019f68c0-ca4d-7c40-8e98-a2ce0771493a` 的同一 Answer Presentation 又分别从 live 与 durable 通道进入兼容 SSE，AionCore 因此收到两次等大的 `agent_message_chunk`。

## 当前决策

- `NativeRunRequest.system/history` 规范化后写入保留的 `__native_model_prompt_context`；它是 runtime 的规范 prompt context，工作流 start input 继续作为兼容投影。
- LLM 的 integration context 从 Native prompt context 构造 `NativeModelInvocationV2`，不再依赖 LLM 与 start node 是否直接相邻；显式 `context_selector` 和 `integration_context=disabled` 语义保持不变。
- Provider 请求仍只从 AI Native 生成；没有新增 raw Anthropic body、协议 envelope 正文或插件侧历史回填。
- Compatible SSE 在协议 mapper 前按 Answer Presentation 的 event type、text、answer node、segment 与 source identity 做单 run exactly-once；Provider raw delta 不参与该去重。
- live delta 与 durable 合并记录的切分边界可以不同；compatible SSE 必须按同一 Answer Presentation identity 累计 durable 文本，再与已发送 live 前缀对账，不能逐个 durable batch 和完整 live 文本做前缀比较。
- subagent callback 的 system 识别优先读取 Native prompt context 的类型化 Text Blocks，旧 run 才回退 start projection。
- `sys / env / conversation / trigger` 是运行级全局命名空间；普通节点输出仅对连接器可达下游可见，编译、首次运行、恢复和预览共用同一契约。
- `orchestration-runtime` 是唯一执行状态机；control-plane 只负责生命周期、持久化、事件和权限，不再保留第二套首次执行循环。
- 多 Answer 图按真实 Provider 来源和 checkpoint active set 选择终点；未激活分支的静态 Answer 不得污染等待态、终态或 SSE block 顺序。
- `sys.dialog_count` 由 Native history 中此前 user turn 数生成；首轮为 0，第二轮为 1，兼容旧输入时才回退 Start history 投影。
- Compatible SSE / Native stream 的终态兜底统一读取 `PublishedRunStreamState`；投影只包含当前 run 的 status、output/error、节点 usage 子字段和最新 pending callback，不得回到 `ApplicationRunDetail`、stitched trace 或 subagent trace。
- Stream state 以初始 `NativeRunResult` 保留稳定身份、metadata 与 node input，只覆盖持久化动态状态；usage 聚合优先级与 pending `llm_tool_calls` required action 契约保持不变。

## 验证证据

- AionUI/Claude Code session `a889e49c-f1b2-4d3e-ad60-e9023c0d2f63` 两轮暗号测试成功：run `019f6917-006b-7851-a954-521d9f270d2e` 建立 `Native-7F3A`，run `019f6918-0333-7691-9e7f-13a68d36d594` 只回复该暗号。
- 第二轮 Native context 有 6 条历史，Provider Invocation 有 7 条消息（历史 + 当前 turn），历史中存在暗号；AionCore 每轮只收到 1 个 `agent_message_chunk`。
- 定向回归：orchestration LLM context 8 passed、control-plane application public API 171 passed、compatible SSE 41 passed、provider contract 23 passed、Rust static gate warnings=0。
- 最终 Claude Code session `3ab4dffb-3d6d-4fc7-85d4-6c9dfe79001b` 两轮成功：第一轮 `已记住`，第二轮只返回 `FLOWBASE-5173`；run `019f69b5-0447-7980-b181-a5d4eef3fc0a` 与 `019f69b6-9e2d-7d60-b543-95e03b83291b` 均 succeeded，第二轮 history=2、dialog_count=1，Answer delta 拼接后无重复。
- 最新自动化证据：orchestration-runtime 253 passed、control-plane orchestration 178 passed、前端变量/预览/节点目录 96 passed、TypeScript 通过、scoped clippy `-D warnings` 通过、Rust static warnings=0。
- AionUI SQLite 对旧 session `a889e49c-f1b2-4d3e-ad60-e9023c0d2f63` 的 text/thinking exact duplicate groups=0；因此未修改 AionUI。
- OOM 复现样本的旧完整详情会展开约 485 MB 历史 JSON；轻量投影只读取约 1.9 KB 当前状态数据。repository 红绿测试、control-plane stream-state contract 测试、compatible SSE 41 项回归和 Rust static gate 均通过；新后端启动后 RSS 约 100 MB。
- 2026-07-17 run `019f6e3d-8148-74d0-8204-cfcea2984990` 证明 64 KiB / 20 ms durable batching 改变 chunk 边界后，旧逐 batch 前缀判断会把 2,263 bytes 再投影一次；新增多 batch 红灯精确复现截图中的残缺重复，累计 durable 前缀修复后 compatible SSE 定向回归 44 passed。

## 截止日期与动机

2026-07-16 已交付；动机是继续坚持 `Anthropic -> AI Native -> Anthropic`，让图拓扑、持久化 replay 和客户端 UI 都不能建立第二份协议真值。
