---
memory_type: project
topic: MCP Debug output_mapping filter semantics
summary: MCP debug execute 的 output_mapping 是对目标接口返回 payload 的过滤器，不是结果契约校验；过滤无法命中时返回完整 payload，目标响应获取或解析问题必须归到 interface_response 边界。
keywords:
  - mcp-management
  - mcp-debug-execute
  - output_mapping
  - interface_response
  - tool_result
match_when:
  - 调整 /api/console/mcp/debug/execute
  - 调整 MCP Tool input_mapping 或 output_mapping 语义
  - 处理 MCP debug execute 返回值、错误码或 tool_result 映射
created_at: 2026-07-05 10
updated_at: 2026-07-05 10
last_verified_at: 2026-07-05 10
decision_policy: verify_before_decision
scope:
  - api/apps/api-server/src/routes/settings/mcp_management/debug_execute.rs
  - api/apps/api-server/src/_tests/mcp_management_debug_execute_routes.rs
  - web/app/src/features/settings/components/mcp-management
  - web/packages/api-client/src/console-mcp-management.ts
---

# MCP Debug output_mapping filter semantics

## 时间

`2026-07-05 10`

## 谁在做什么

用户确认 MCP debug execute 的 `output_mapping` 只负责对目标接口返回 payload 做剪裁过滤，不负责把工具结果统一校验成某个强契约。

## 为什么这样做

`output_mapping` 本质是工具调试阶段的结果过滤。目标接口已经成功返回时，过滤字段缺失或无法形成过滤结果不应该把一次成功的目标调用改写成 `400 output_mapping`。

## 已确认语义

- 目标接口非 2xx 时仍透传目标接口响应。
- 目标接口成功且 response 可解析时，`tool_result` 从目标返回 payload 过滤得到。
- `output_mapping.properties` 有部分字段命中时，只返回命中的字段。
- `output_mapping.properties` 全部未命中、为空，或无法作为过滤结构使用时，返回完整 payload。
- 目标响应获取或 JSON 解析异常不归因到 `output_mapping`，应暴露在 `interface_response` 或目标响应获取边界。

## 决策背后动机

避免把“过滤配置没有命中字段”和“目标接口返回值不可用”混成同一个错误。调用方应始终能看到目标接口实际返回的 payload，debug execute 只在目标请求构造、认证权限、目标接口失败或目标响应获取失败时返回真正错误。
