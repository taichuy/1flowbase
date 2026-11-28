---
title: "@taichuy/workspaces 多租户商业插件二次重构与底座数据隔离方案对齐"
memory_type: project
created_at: "2026-11-23 00"
updated_at: "2026-11-23 21"
last_verified_at: "2026-11-23 21"
decision_policy: verify_before_decision
status: active
scope:
  - api
  - web
  - /home/taichuy/git/1flowbase-plugins-business/runtime-extensions/@taichuy/workspaces
tags: [workspaces, multi-tenant, saas, commercial-plugin, onboarding, data-isolation]
---

# `@taichuy/workspaces` 多租户商业插件二次重构与底座数据隔离方案对齐

- **谁在做什么**：基于用户最新收敛，敲定 `1flowbase` 底座第 1 层空间数据隔离加固 + `@taichuy/workspaces` (`v0.2.0`) 普通受管设置插件方案（确认前不改产品代码）。
- **用户最新明确需求与边界（2026-11-23 21）**：
  1. **不新增内置角色，对齐 `@taichuy/ssh` 普通受管设置插件模式**：删去 `workspace_manager` / `builtin_roles` 设计；`@taichuy/workspaces` 作为标准受管设置插件，由后端收敛为**仅限 `root` 空间的角色可配置与使用**（非 `root` 空间不展示、不可配置、不可调用；`root` 空间内按角色接口权限控制侧边栏显隐与访问）。
  2. **先哪一个空间 → 后什么角色 + 底座数据硬隔离**：先闭合底座多空间数据隔离（禁非 `root` 空间 `system_all` 全库读穿、`users` 模型按 `workspace_memberships` 过滤、修复 `RoleRepository`/`ModelDefinitionRepository`/MCP/文件表多空间切换串回首个空间、宿主全局平台设置收口至 `root` 空间）；其他外部插件与 SQL 节点查询不在本次范围。
  3. **扁平工作空间主表与管理员查增删 / 批量指定**：主表 `1 行 = 1 个工作空间`，提供单空间「管理管理员」Drawer（查/增/删）与多选「批量指定管理员」。
  4. **Root 空间新用户入驻设置 & 暂缓邀请**：支持 `default_workspace`（默认进入指定空间）、`personal_workspace`（自动独立空间）、`reject`（拒绝自行加入）；本轮移除邀请功能。
- **为什么要做与决策动机**：用最简正交两层模型（空间定数据与平台挂载边界、角色定空间内接口交叉权限）取代冗余内置角色与硬编码特判，同时堵死底座跨租户读穿与串写漏洞。
- **截止日期**：待用户确认本最终简化计划后立即实施并发布 `workspaces-v0.2.0`。
