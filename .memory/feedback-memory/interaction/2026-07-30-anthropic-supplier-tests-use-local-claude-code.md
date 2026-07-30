---
feedback_category: interaction
decision_policy: direct_reference
created_at: 2026-07-30 12
summary: 对声明只允许 Claude Code 的 Anthropic 供应商，真实模型请求只能经本地 Claude Code 的 ACP 或 tmux 发起。
---

# Anthropic Supplier Tests Use Local Claude Code

## Rule

排查或验收明确限制客户端的 Anthropic 供应商时，真实模型请求只使用机器现有的本地 Claude Code，并通过 ACP 或 tmux 运行；不要用 curl、SDK、Provider 二进制或自写 HTTP 请求直接调用供应商 API。

## Reason

供应商会按客户端行为或协议特征限制调用。绕过 Claude Code 的测试既可能被拦截，也不能证明用户真实链路可用。

## Applies When

- 用户说明某个 Anthropic 供应商仅允许 Claude Code。
- 对比 Claude Code 原生配置、cc-switch 配置与 1flowbase Anthropic 兼容网关。
- 需要做供应商侧复现、修复后验证或回归。

## Practice

DNS、TCP、TLS、进程环境和本地持久化日志可独立做只读诊断；任何会到达模型 API 的请求仍必须从本地 Claude Code 发起。
