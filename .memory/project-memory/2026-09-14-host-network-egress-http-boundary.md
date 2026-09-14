---
memory_type: project
topic: Host 统一管理 API 出站 HTTP 与 GitHub 官方源门禁
summary: 用户确认复用闭合 github consumer，由 Host Network Egress 统一提供 HTTP client；业务模块不得自行直连，repo-hygiene 负责阻止新增旁路。
keywords:
  - network-egress
  - github-official-sources
  - outbound-http
  - repo-hygiene
  - fail-closed
created_at: 2026-09-14 17
updated_at: 2026-09-14 17
last_verified_at: 2026-09-14 17
decision_policy: verify_before_decision
scope:
  - api/apps/api-server
  - scripts/node/repo-hygiene
status: implemented
---

# Host 统一管理 API 出站 HTTP 与 GitHub 官方源门禁

2026-09-14 17，用户确认由 Codex 实现平衡方向：Host-owned Network Egress 是 API 出站 HTTP 的统一 client 边界，GitHub 官方来源复用既有 `GithubOfficialSources` consumer，不按计费、i18n、版本检查、MCP Bundle、UI 组件等业务模块重复注册。

这样做是为了让代理路由、失败策略和租约生命周期由一个宿主边界管理，避免新增模块时再次遗漏代理。命中已配置路由后，代理池或租约失败必须 fail closed，不能回退直连；只有没有启用路由时保留直连行为。同一次多文档目录操作（例如 index 与 page）共用一个请求 scope，并在结束时释放对应租约。

实现同时用 `repo-hygiene` 的 `unowned-outbound-http-client` error 门禁阻止业务生产代码直接构造 `reqwest::Client`。只有 Network Egress owner、已接入路由的官方扩展目录、仅测试使用的遗留 registry，以及带 SSRF/DNS pinning 职责的第三方 MCP upstream 安全边界进入带原因的窄白名单。

该决策无截止日期。后续若新增独立的出站路由语义，先确认新的 consumer selector；普通 GitHub 官方源模块应复用现有 consumer 和 Host 请求 scope，不能新增模块级 client 或平行代理配置。
