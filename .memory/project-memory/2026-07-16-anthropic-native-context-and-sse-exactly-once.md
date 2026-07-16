---
memory_type: project
topic: Anthropic Native 上下文与兼容 SSE exactly-once 修复
summary: Claude Code/AionUI 续聊历史已通过保留的 Native prompt context 跨工作流节点进入 Provider Invocation；兼容 SSE 会按 Answer Presentation 语义身份消除 live/durable 重复投影，Anthropic -> AI Native -> Anthropic 仍是唯一执行链路。
keywords:
  - anthropic
  - ai native
  - claude code
  - aionui
  - conversation history
  - sse
  - exactly once
created_at: 2026-07-16 12
updated_at: 2026-07-16 12
last_verified_at: 2026-07-16 12
decision_policy: verify_before_decision
status: delivered
scope:
  - api/crates/plugin-framework/src/provider_contract.rs
  - api/crates/control-plane/src/application_public_api
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
- subagent callback 的 system 识别优先读取 Native prompt context 的类型化 Text Blocks，旧 run 才回退 start projection。

## 验证证据

- AionUI/Claude Code session `a889e49c-f1b2-4d3e-ad60-e9023c0d2f63` 两轮暗号测试成功：run `019f6917-006b-7851-a954-521d9f270d2e` 建立 `Native-7F3A`，run `019f6918-0333-7691-9e7f-13a68d36d594` 只回复该暗号。
- 第二轮 Native context 有 6 条历史，Provider Invocation 有 7 条消息（历史 + 当前 turn），历史中存在暗号；AionCore 每轮只收到 1 个 `agent_message_chunk`。
- 定向回归：orchestration LLM context 8 passed、control-plane application public API 171 passed、compatible SSE 41 passed、provider contract 23 passed、Rust static gate warnings=0。

## 截止日期与动机

2026-07-16 已交付；动机是继续坚持 `Anthropic -> AI Native -> Anthropic`，让图拓扑、持久化 replay 和客户端 UI 都不能建立第二份协议真值。
