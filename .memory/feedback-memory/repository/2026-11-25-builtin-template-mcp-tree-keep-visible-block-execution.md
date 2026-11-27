---
memory_type: feedback
feedback_category: repository
topic: 内置应用模板自带的 MCP 目录树在宿主策略禁用时保持可见并在执行期拦截
summary: 被内置应用模板（如 @taichuy/gateway-demo）消费的宿主能力（如 /terminal 本地终端工具树），在通过部署环境变量禁用时不应隐藏 MCP 目录树或改为不可绑定，而应在执行入口 fail-closed 返回纯英文提示。
keywords:
  - mcp
  - terminal
  - gateway-demo
  - application-template
  - deployment-policy
match_when:
  - 为被内置应用模板引用的宿主接口或 MCP 工具增加部署级禁用开关
  - 评估禁用某类内置接口时是否要同步隐藏 MCP 目录树或修改 bindable
created_at: 2026-11-25 17
updated_at: 2026-11-25 17
last_verified_at: 2026-11-25 17
decision_policy: direct_reference
scope:
  - api
  - /home/taichuy/git/1flowbase-official-plugins/applications-demo/@taichuy/gateway-demo
---

# 内置应用模板自带的 MCP 目录树在宿主策略禁用时保持可见并在执行期拦截

## 时间

`2026-11-25 17`

## 规则

- 当某个宿主本机能力（如 `/terminal` 本地命令与终端工具树）已被内置应用模板（如 `1flowbase-official-plugins/applications-demo/@taichuy/gateway-demo`）内置携带时，新增环境变量禁用开关不应在 MCP 目录发现层隐藏该工具树或将其改为 `bindable=false`，而应保持目录可见并在 Canonical 执行入口统一拦截，同时在 HTTP 与 MCP 调用错误中返回纯 ASCII 英文提示指明具体开关（避免编码乱码）。

## 原因

- 内置应用模板默认自带这些分组与工具绑定，目录可见属于模板结构的一部分；隐藏目录会造成模板表现不一致或影响模板同步，而执行期硬拦截配合明确英文错误原因即可同时兼顾云端外部 `root` 安全隔离与可诊断性。

## 适用场景

- 云端独立部署给外部租户（含独立 `root`）时通过后端环境变量限制宿主机敏感接口。
- 涉及内置应用模板或内置 MCP 包所绑定接口的部署策略开关设计。
