---
memory_type: project
topic: DSH 经 AI Gateway 的工具协议兼容与连续 callback recovery
summary: 用户已确认同协议能力可表达时保留协议语义、AI Native 或跨供应商路径进行语义翻译的边界，并批准 Single Issue #1736 修复 Anthropic tool 字段兼容与 waiting_callback recovery 状态冲突。
keywords:
  - DeepSeek Harness
  - AI Gateway
  - Anthropic tool
  - eager_input_streaming
  - strict
  - defer_loading
  - waiting_callback
  - issue 1736
created_at: 2026-08-16 17
updated_at: 2026-08-16 18
last_verified_at: 2026-08-16 18
decision_policy: verify_before_decision
status: user_acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1736
  - /home/taichuy/git/1flowbase
  - /home/taichuy/git/deepseek-harness
---

# DSH Gateway Tool And Recovery Issue

## 谁在做什么

Issue #1736 已完成 TDD、后端与 official Anthropic Provider 实现、集中 QA 和本地合并。主仓 `dev` 集成提交为 `20f32bad0`，official plugins `main` 为 `073713d`；未 push，等待用户部署并完成 DSH 双协议人工验收。

## 为什么这样做

两个失败分别发生在 Anthropic ingress 字段识别和 durable recovery 状态写入，但共同阻断 DSH 通过已发布 AI Native Gateway 完成多轮工具调用。一个 Single Issue 用同一双协议端到端验收闭环，同时保留独立 AC，避免把两个根因混为一个翻译问题。

## 已确认边界

- 同一 wire protocol 且下游 Provider capability 能表达字段时，保留协议语义并尽量透传。
- 进入 AI Native 工作流或跨供应商时，先归一化为 AI Native 语义，再由 Provider 渲染供应商协议；不承诺无条件 raw passthrough。
- `eager_input_streaming` 支持时保留，不支持时允许可观察的受控降级；`strict`、`defer_loading` 等真实执行语义必须 capability mapping 或 typed rejection，不得静默丢弃。
- callback recovery 真值时间线为 `waiting_callback → running → waiting_callback → running → succeeded`；由 durable storage owner 在 resume claim 时原子追加 `running` 事实，保留严格 PostgreSQL trigger。
- AgentFlow 注入 `image_llm` 不作为缺陷；只验证内部工具不会与客户端工具重名或错误进入客户端 callback。

## 为什么要做

目标是让 DSH 的 Anthropic Messages 和 OpenAI Chat 两条入口都能经 DeepSeek Provider 完成连续工具调用，同时维持 AI Native、Provider 和 durable storage 的 owner 边界，而不是通过客户端补偿、放宽数据库约束或修改 DeepSeek Provider 掩盖上游缺陷。

## 截止日期与停止条件

无固定截止日期。自动化候选已通过；停止条件为等待用户部署后验证 DSH Anthropic Messages 与 OpenAI Chat 连续工具调用。标题请求 reasoning 截断仍不纳入 #1736。

## 决策动机

遵循第一性原理按字段是否改变执行语义分类，按奥卡姆剃刀把两个独立根因分别修复，并用一次 DSH 双协议集中验收证明最终行为。
