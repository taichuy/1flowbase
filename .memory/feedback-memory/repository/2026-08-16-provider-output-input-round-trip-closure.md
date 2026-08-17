---
memory_type: feedback
feedback_category: repository
topic: Provider 输出到后续输入必须满足 round-trip closure
summary: 诊断兼容协议多轮 reasoning 时，必须区分展示历史、完整 canonical 审计历史和 Provider 输入投影；系统允许返回的 Provider 输出应能在下一轮原生恢复或安全降级，不能把不可回放 reasoning 自动升级为整条 route 的 hard capability。
keywords:
  - round-trip closure
  - reasoning history
  - provider projection
  - replayability
  - continuation
  - canonical history
match_when:
  - Provider 首轮能输出 reasoning、下一轮却因 reasoning_history capability 被拒绝
  - 兼容协议把 assistant reasoning 回传到 AI Native
  - 设计 canonical history、Provider-bound projection 或 continuation contract
created_at: 2026-08-16 23
updated_at: 2026-08-16 23
last_verified_at: 2026-08-16 23
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - api/crates/plugin-framework
  - api/crates/control-plane/src/application_public_api/compat
  - ../1flowbase-official-plugins/runtime-extensions/model-providers
---

# Provider 输出到后续输入必须满足 Round-trip Closure

## 规则

系统允许产生、返回或持久化的 Provider 输出语义，必须满足下一轮闭包：要么通过同一 Provider 的原生 continuation 恢复，要么由 Provider 对完整 canonical 原文生成安全的输入投影并返回 `exact / lossy / unsupported` receipt。普通 assistant reasoning 在已有可见最终回答时可以安全省略；不可因为目标 Provider 不接受 reasoning history 就拒绝整个会话。

展示历史、平台完整 canonical 审计历史和 Provider 实际输入历史是三个对象。AI Native 拥有统一 semantic、replayability/fidelity contract 与完整 canonical 真值；Provider 插件拥有具体 wire lowering、输入/输出/continuation 能力及 projection receipt；Router 根据 receipt 和策略选 route，每次重试从完整 canonical 原文重新投影。

`reasoning_redacted`、加密或签名 reasoning 没有原生 affinity/continuation 时默认不回放，也不得伪装成普通文本。能力必须至少区分 reasoning output、reasoning history input 与 native continuation，不能使用一个笼统 reasoning capability。

## 原因

用户纠正：把此类失败仅解释为跨供应商不兼容漏掉了更基础的 round-trip closure 断裂。输出可用但回放不可用，且不可回放语义在 Provider projection 前被推导为 hard route capability，会让同一 Provider 的第二轮也失败。

## 适用场景

- 兼容入口在 streaming/non-streaming 输出 reasoning，并在下一轮接收客户端回传历史。
- 设计 OpenAI Responses `previous_response_id`、ProviderContinuation 或显式 full-history replay。
- 调整 route capability filter、Provider manifest、reasoning replay policy、projection receipt 和相关自动化测试。
