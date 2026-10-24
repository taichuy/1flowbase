---
memory_type: feedback
feedback_category: interaction
topic: real-multi-agent-responses-soak
summary: Responses 客户端排障使用本机 Codex/tmux 补齐真实交互与重试，不以自制客户端替代；长测保持真实多代理能力，覆盖子线程和网关收尾。
keywords:
  - Responses
  - Codex CLI
  - multi_agent
  - long task
match_when:
  - 使用 Codex CLI 对 1flowbase Responses 网关做真实长任务验证
  - 为规避子线程错误考虑关闭多代理能力
created_at: 2026-09-23 19
updated_at: 2026-10-24 00
last_verified_at: 2026-10-24 00
decision_policy: direct_reference
scope:
  - gateway verification
  - long-running Codex CLI tasks
---

# Responses 长测保留真实多代理能力

## 规则

长任务测试使用真实 Codex CLI 配置，保持多代理能力可用；不得用 `features.multi_agent=false` 绕开子线程路径。记录父线程和子线程的请求、回调、取消及错误时序。

实机故障优先依据开源源码、协议和日志定位。本轮用户明确排除直接二进制分析和内存分析，后续延续同一长测时沿用此边界；普通进程、端口和系统资源检查仍可用于运行管理。不要为诊断偏离源码优化和真实任务验证。

## 原因

用户需要发现真实多代理调用中的协议缺陷。关闭多代理会缩小被测路径，使长测通过不能证明目标环境稳定。用户同时要求避免二进制／内存分析干扰上游任务；该担忧是用户设定的测试边界，不据此断言上游会如何识别或暂停请求。

## 真实客户端排障补充

- 排查 Codex 的错误展示、自动重试、重连或传输降级时，若本机已有 Codex/tmux，必须补真实客户端对照；自制协议客户端可定位边界，但不能替代该验证，也不能仅列“未用官方客户端”便收尾。
- 使用独立 CODEX_HOME、私有 tmux socket 和专用测试资源，不修改用户现有配置或中断已有会话。
- 原因：真实客户端可能有自制 fixture 未覆盖的后台推理和重试行为；客户端证据缺口与无法取得远端历史槽位日志是两件事，不能混作环境阻塞。

## 弱网并发排障的主线边界

- 规则：用户明确将标题等额外独立推理移出本次弱网并发问题主线；优先验证两个固定会话的断开、自动重试、手动继续，以及网关和远端分别何时释放调用。已知客户端可能发额外请求，不足以据此解释客户事件。
- 原因：需要定位直连与经过网关的生命周期差异；应区分客户端请求数、活动推理数和 sub2api 用户并发槽，不以无法确认的后台工作替代网关职责诊断。
- 适用场景：延续 2026-10-23 客户 0.5.8 的弱网并发排障和相关修复验证；实验容量、hold/半开注入与客户真实条件分开报告。

## 适用场景

1flowbase Responses 网关的 Codex CLI 故障复现、长任务和回归验收。
