---
memory_type: feedback
feedback_category: interaction
topic: real-multi-agent-responses-soak
summary: Responses 长任务验证保持 Codex 真实多代理能力，不设置 features.multi_agent=false；子线程的创建、取消和网关收尾也是验收范围。
keywords:
  - Responses
  - Codex CLI
  - multi_agent
  - long task
match_when:
  - 使用 Codex CLI 对 1flowbase Responses 网关做真实长任务验证
  - 为规避子线程错误考虑关闭多代理能力
created_at: 2026-09-23 19
updated_at: 2026-09-23 19
last_verified_at: 2026-09-23 19
decision_policy: direct_reference
scope:
  - gateway verification
  - long-running Codex CLI tasks
---

# Responses 长测保留真实多代理能力

## 规则

长任务测试使用真实 Codex CLI 配置，保持多代理能力可用；不得用 `features.multi_agent=false` 绕开子线程路径。记录父线程和子线程的请求、回调、取消及错误时序。

## 原因

用户需要发现真实多代理调用中的协议缺陷。关闭多代理会缩小被测路径，使长测通过不能证明目标环境稳定。

## 适用场景

1flowbase Responses 网关的 Codex CLI 长任务和回归验收。
