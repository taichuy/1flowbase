---
memory_type: feedback
feedback_category: repository
topic: 真实客户端直连成功而 Gateway 失败时先做 wire 差分
summary: 同机同客户端直连成功且 Gateway 同时段稳定失败时，不能因为 Gateway 还存在代理配置问题就把根因收敛到网络；必须先对齐配置时间线，并比较 ingress、provider wire、上游原始状态与客户端结果。
keywords:
  - Claude Code
  - wire fidelity
  - differential diagnosis
  - proxy attribution
  - Anthropic Gateway
match_when:
  - 同一真实客户端直连供应商成功但经过 AI Gateway 失败
  - 供应商按客户端、协议或传输指纹限制访问
  - 同时存在代理缺失、429、503 或网络不稳定迹象
created_at: 2026-07-30 13
updated_at: 2026-07-30 13
last_verified_at: 2026-07-30 13
decision_policy: direct_reference
scope:
  - AI Gateway incident diagnosis
  - Anthropic compatible ingress and provider adapters
---

# 真实客户端直连成功而 Gateway 失败时先做 Wire 差分

## 规则

- 同一机器、同一客户端版本、同一供应商在重叠时段出现“直连成功、Gateway 失败”时，网络或供应商全局故障不是充分解释；先建立客户端会话、FlowRun、provider attempt 与出口日志的时间对齐。
- 代理配置缺失可以作为独立缺陷修复，但不能据此停止协议排查。必须确认失败样本发生时的配置版本，并在配置修复后重新通过真实客户端复现。
- 根因收敛至少比较：认证呈现方式、客户端身份/session header、beta header 的值与多值形态、system/messages/tools/content block、JSON/header 顺序、HTTP/TLS client，以及上游原始 status/body。
- 外层状态码与错误正文语义不一致时，先从 provider details/FlowRun 判断上游原始状态；不要先假定 Gateway 做了错误映射。

## 原因

Claude Code-only 供应商可能同时参考 HTTP 语义、header/body 形态和传输指纹。代理问题与 wire fidelity 问题可以同时存在；只修复前者会产生错误的成功结论，并掩盖真实客户端身份在 Gateway 重建过程中丢失的问题。

## 适用场景

- Claude Code、Codex、OpenCode 的原生配置与 1flowbase Gateway 配置做 A/B。
- Anthropic-compatible 上游返回 429/503、`Service Unavailable` 或客户端身份限制错误。
- 需要判断问题属于本地代理、供应商并发限制、错误投影还是协议/传输重建。
