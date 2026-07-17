---
memory_type: feedback
feedback_category: repository
topic: dev-up 必须以 worktree 本地环境配置为真值
summary: 多个本地 worktree 并行开发时，dev-up 必须读取并保留各自的 env 端口、代理和 database 配置；拆分 env 文件时要迁移既有值，不能回退到主工作区默认端口。
keywords:
  - dev-up
  - worktree
  - env
  - port isolation
  - database isolation
match_when:
  - 修改 scripts/node/dev-up
  - 新增或拆分本地环境配置文件
  - 本地多个工作区同时启动
created_at: 2026-07-17 23
updated_at: 2026-07-17 23
last_verified_at: 2026-07-17 23
decision_policy: direct_reference
scope:
  - scripts/node/dev-up
  - api/apps/api-server/.env
  - web/app/.env
---

# dev-up 必须以 worktree 本地环境配置为真值

## 规则

`dev-up` 启动本地服务时必须读取当前 worktree 自己的环境配置。`1flowbase_latest` 与 `../1flowbase` 可以复用 PostgreSQL 容器，但必须使用不同 database、服务端口和浏览器 cookie 名。

把前端变量从 API `.env` 拆到 `web/app/.env` 时，首次生成 Web `.env` 必须继承既有的 `VITE_DEV_SERVER_PORT` 与 `VITE_API_PROXY_TARGET`；不能复制默认 `3100/7800` 后覆盖已经生效的 worktree 配置。

## 原因

用户纠正：“不是直接读取环境配置文件吗？”实际故障中，API `.env` 已配置 `3200/7900`，新生成的 Web `.env` 却回退到 `3100/7800`，造成 Vite 端口冲突并把当前前端代理到另一个工作区的旧后端。

## 适用场景

修改本地启动、进程状态、端口探活、env 模板拆分或多 worktree 开发环境时命中。
