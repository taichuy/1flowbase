---
memory_type: project
topic: published-agent-model-capability-contract-issues
summary: 用户确认已发布 agent 的对外模型能力应由 Start 节点 `model_list` 配置作为真源；模型窗口默认按最大窗口、输出按 1/16 向上归档、自动压缩按 85% 联动，用户自定义后保持。
keywords:
  - Start model_list
  - published agent model capability
  - context_window
  - auto_compact_token_limit
  - reasoning
  - /api/agent/v1/models
  - /api/agent/v1/runs
match_when:
  - 设计或实现对外 Agent 模型发现接口
  - 修改 Start 节点模型列表配置
  - 处理外部客户端自动压缩或 reasoning 参数传入
  - 接续 GitHub issue #542-#548
created_at: 2026-05-30 00
updated_at: 2026-07-31 23
last_verified_at: 2026-07-31 23
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/542
  - https://github.com/taichuy/1flowbase/issues/543
  - https://github.com/taichuy/1flowbase/issues/544
  - https://github.com/taichuy/1flowbase/issues/545
  - https://github.com/taichuy/1flowbase/issues/546
  - https://github.com/taichuy/1flowbase/issues/547
  - https://github.com/taichuy/1flowbase/issues/548
---

# Published Agent Model Capability Contract Issues

## 时间

`2026-05-30 00`

## 谁在做什么

用户确认：已发布 agent 的对外模型能力配置应挂在 Start 节点 `model_list` 上。AI 已创建 GitHub issue 树 #542-#548，随后用户要求直接进入实现并保持当前 `main` 分支不切换。当前工作树已实现后端模型发现投影、runs reasoning 参数校验与冻结、LLM 节点外部 reasoning opt-in、Start 节点模型配置 UI、前端新增表单 `flowbase` 预填默认、后端内置 `1flowbase` fallback、百分比自动压缩阈值和 `userinput.reasoning_effort` Start 变量。

## 为什么这样做

外部 agent 客户端需要在创建 run 之前拿到该已发布应用编排后的大语言模型上下文窗口和自动压缩阈值。这个能力是用户为该 agent 工作流配置的发布态模型目录，不应由客户端在 run 时传入，也不应由网关从全局模型仓库猜测。

## 为什么要做

如果模型能力不跟 Start 配置绑定，Codex、Hermes、opencode 等客户端无法稳定发现上下文窗口，自动压缩和运行时 reasoning 参数也会缺少明确的 source of truth。

## 截止日期

无固定截止日期；当前实现已在本地工作树完成，交付前需关注仓库既有 TypeScript、i18n hygiene、Rust static gate 阻塞项是否由用户另行处理。

## 决策背后动机

- Start `model_list` 是已发布 agent 的模型能力目录真源。
- `GET /api/agent/v1/models`、Codex `/models?client_version=...`、OpenAI-compatible `/v1/models`、opencode/Hermes 投影都从该目录派生。
- `context_window`、`max_output_tokens`、`auto_compact_token_limit` 和 `capabilities` 是模型静态能力，不作为普通运行时变量。
- `POST /api/agent/v1/runs` 只接 `execution.model_parameters.reasoning` 这类运行时偏好。
- 外部客户端传入的 reasoning effort 字符串属于用户输入上下文，后续节点通过 Start 输出 `userinput.reasoning_effort` / `node-start.reasoning_effort` 获取；`sys` 只保留运行时内部需要的 `model_parameters`。
- LLM 节点默认使用自身参数；只有显式 opt-in 后才跟随外部 reasoning 参数。
- 前端新增模型表单可预填默认值，但只有用户保存后才写入 Start `model_list`。
- 前后端通用默认规则：`context_window = max_context_window`；`max_output_tokens` 取 `context_window / 16` 向上归入 `4K / 8K / 16K / 32K / 64K`；`auto_compact_token_limit = round(context_window * 85%)`。内置 128K fallback 因此为 `128K / 128K / 8K / 108.8K`。
- Start 表单只在当前值精确等于旧派生默认时继续联动；用户自定义后保持，改回精确默认后恢复联动。`context_window > max_context_window` 时表单报错并禁止保存，不静默改写用户值。
- `allow_external_override` 不属于模型发现能力字段，已从 Start 模型表单、flow schema、后端 DTO 和公开文档中移除；客户端能否传 reasoning 由 runs 契约天然支持，应用到 LLM 节点由 `follow_external_reasoning` 控制。

## Issue 树

- L0 #542：对外 Agent 模型能力发现与 reasoning 参数契约
- L1 #543：确认 Start `model_list` 作为已发布 Agent 模型能力真源
- L2 #544：发布 Agent 模型发现与 runs reasoning 参数后端契约
- L2 #545：Start 模型能力配置与 LLM 外部 reasoning 策略
- L3 #546：实现已发布 Agent 模型能力发现后端投影
- L3 #547：实现 `runs.execution.model_parameters.reasoning` 到 LLM 节点 opt-in
- L3 #548：实现 Start 模型能力配置与 LLM reasoning 跟随开关
