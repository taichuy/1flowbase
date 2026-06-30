---
memory_type: project
topic: workflow balanced design direction approved
summary: 用户已确认 workflow 第一版走 balanced 方向：作为 ApplicationType::Workflow 的正式产品形态，复用现有 orchestration runtime / logs / monitoring，但拥有 workflow 专属 UI、触发器、同步/异步运行、全局唯一 /api/ex/{slug} 发布契约、全局 OpenAPI 注册，以及 workflow_start / workflow_end 等应用类型专属节点命名空间。
keywords:
  - workflow
  - application
  - orchestration-runtime
  - api-ex
  - workflow_start
  - workflow_end
  - trigger
match_when:
  - 需要设计或实现 workflow 产品形态
  - 需要判断 workflow 是否复用 AgentFlow start/answer 节点
  - 需要设计 /api/ex/{slug} 或 workflow 同步/异步触发契约
created_at: 2026-06-30 00
updated_at: 2026-06-30 01
last_verified_at: 2026-06-30 01
decision_policy: verify_before_decision
scope:
  - api
  - web
  - workflow
  - application public API
---

# workflow balanced design direction approved

## 时间

`2026-06-30 00`

## 谁在做什么

用户准备在 AgentFlow 稳定后启动 Workflow 产品形态。AI 先按 `problem-framing` 对齐方案，用户确认 balanced 方向合理，并进一步明确 workflow 的开始 / 结束节点不复用 AgentFlow 的 `start` / `answer` 命名，而应使用 `workflow_start`、`workflow_end` 这类应用类型专属节点命名；后续 workflow 专属节点也必须用区分命名，避免和 AgentFlow 节点重叠。

## 为什么这样做

Workflow 的定位是系统工作流内置工作补充，支持定时、API、自定义 `/api/ex/{slug}`、同步 / 异步运行、自定义同步返回值和专属日志 / 监控。复用现有 orchestration runtime 能降低第一版落地成本，但节点命名、UI 侧栏、触发器和公开 API contract 必须保持 Workflow 产品语义，避免把 AgentFlow 的聊天 / Agent API 概念泄漏到 Workflow。

## 决策

- Workflow 作为 `ApplicationType::Workflow` 的正式产品形态进入 issue gate。
- 第一版走 balanced 方向：复用现有 orchestration runtime / logs / monitoring，不新建平行顶层资源。
- Workflow UI 侧栏只保留“工作流、日志、监控”；API 触发、定时触发、自定义接口配置放在“工作流”页内。
- Workflow 使用应用类型专属节点，例如 `workflow_start`、`workflow_end`；不复用或重名 `start` / `answer`。
- `/api/ex/{slug}` 由 workflow 发布配置承载，slug 全系统唯一；创建 / 保存扩展接口触发器时必须验证冲突，已存在则失败。
- workflow 没有独立 API 页；扩展接口必须注册到全局 OpenAPI。
- 扩展接口触发器支持全部 HTTP 方法；参数来源覆盖 URL/path、query、form、body，参考 settings/mcp-management 的 interface 参数抽取方式。
- 扩展接口认证方式和当前扩展接口保持一致，不为 workflow 另造新认证模式。
- `workflow_start` 定义输入参数和同步超时时间；同步模式返回 `workflow_end` 定义的字段对象，不额外包 `data`；异步模式返回 `run_id/status`；同步超时或等待状态可降级为 `202 + run_id/status`。
- 定时触发第一版创建异步 run；失败即记录失败，不做复杂重试、死信或补偿。
- Workflow 线上 issue 已创建并更新：L0 #1186、L1 #1187、L2 #1188/#1189、L3 #1190/#1191/#1192/#1193/#1194。

## 截止日期

该记忆在 Workflow 相关 issue gate、ADR、L2/L3 实现拆分和后续开发期间有效；若用户后续改为独立 workflow runtime 或放弃 `/api/ex/{slug}`，需要更新。
