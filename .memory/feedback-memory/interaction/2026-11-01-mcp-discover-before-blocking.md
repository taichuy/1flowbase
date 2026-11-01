---
memory_type: feedback
feedback_category: interaction
topic: mcp-discover-before-blocking
summary: 用户指出 1flowbase 无现成 Tool 时应继续使用后台接口发现与接口转 Tool 能力，而非立即宣告无法执行。
created_at: 2026-11-01 16
updated_at: 2026-11-01 16
decision_policy: direct_reference
---

## 规则
用户要求通过 1flowbase MCP 完成业务操作时，没有现成 Tool 不等于无法完成。继续核对后台设置接口、interface 的 bindable 与契约，并在已授权任务边界内复用或转换、挂载和验证 Tool。

## 原因
2026-11-01 用户纠正了仅因没有角色 Tool 就停止的判断；本轮实际转换并调用角色查询与创建均成功。

## 适用场景
已有 MCP 接口发现与 Tool 配置能力、但缺少业务 Tool 的任务。真实阻断必须落实到具体接口的 disabled_reason、权限拒绝或执行错误；不能把目录缺失当作授权不足。
