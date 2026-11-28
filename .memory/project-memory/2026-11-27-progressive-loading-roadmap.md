---
memory_type: project
topic: 后台渐进式加载分阶段治理路线（MCP 优先 + 共享 OpenAPI 内核）
summary: 已确认采用平衡方向：先闭环完成 MCP 管理页全链路渐进式加载、共享 capability_catalog 索引/单条物化解耦及 agent-flow/webmcp/assistant 脱离全量 catalog（同时保留旧接口向后兼容），随后按固定梯队顺序治理其余同类页面。
keywords:
  - mcp-management
  - progressive-loading
  - capability-catalog
  - pagination
  - roadmap
match_when:
  - 处理 /settings/mcp-management 渐进式加载改造或验收
  - 排期或实施后续后台页面渐进式加载（角色权限、模型供应商、UI 管理、认证中心、Docs 动态构建）
  - 涉及 capability_catalog 或 MCP catalog 兼容边界决策
created_at: 2026-11-27 18
updated_at: 2026-11-27 18
last_verified_at: 2026-11-27 18
decision_policy: verify_before_decision
scope:
  - api/crates/api/kernel/src/contracts/openapi_interface
  - api/crates/api/mcp
  - web/packages/api-client/src/console/administration/mcp-management*
  - web/app/src/features/settings/components/mcp-management
  - web/app/src/features/agent-flow/api/mcp-instance-options.ts
---

# 后台渐进式加载分阶段治理路线

## 时间

`2026-11-27 18`

## 谁在做什么

当前阶段先以 Single Issue 闭环完成第一批（MCP 管理页全链路渐进式加载 + 共享内核 `capability_catalog` 解耦 + `agent-flow / webmcp / assistant` 脱离全量 `mcp/catalog`）；后端由远端 `taichuy-dev` Codex（`gpt-6.2-sol`，`xhigh`）按冻结契约实现与验证，前端本地实现并双向同步验收。

## 为什么这样做

- 打开线上 `/settings/mcp-management?tab=instances` 时，顶层壳层阻塞式并发拉取全量 `/api/console/mcp/catalog`（5 张表 + 完整工具 schema/mapping/长文档 + 重建全站 OpenAPI catalog）与 `/api/console/mcp/interface-capabilities?bindable_only=false`（再次重建全站 OpenAPI catalog 并返回全部接口完整 schema）。
- 共享内核 `build_openapi_capability_catalog_with` 存在“先全量构造再切片/单条查找”的根因，解耦后可同时惠及 MCP、Frontstage、Event Center 与工具调用热路径。

## 为什么要做

遵循“功能完整性 > 稳定性 > 性能 > 可演进性”：
1. 保留 `GET /api/console/mcp/catalog` 与无参 `GET /api/console/mcp/interface-capabilities` 向后兼容，不破坏既有调用方；
2. 第一批收口后，已确认的后续治理顺序为：**角色权限页 → 模型供应商页 & 画布徽章 → UI 管理源码列表 & 认证中心概览 → Docs 动态扩展重建**。

## 截止日期

无固定日历截止日期；按批次逐个验收关闭。

## 决策背后动机

避免一次性大爆炸重构跨 10+ 业务域带来的回归风险，先用 MCP + 共享内核建立标杆闭环，再按影响面梯队推进同类模块。

## 关联文档

- `api/crates/api/kernel/src/contracts/openapi_interface/capability_catalog.rs`
- `api/crates/api/mcp/src/routes/settings/mcp_management.rs`
- `web/app/src/features/settings/pages/settings-page/SettingsMcpManagementSection.tsx`
