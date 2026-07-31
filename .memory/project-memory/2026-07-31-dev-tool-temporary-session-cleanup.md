---
memory_type: project
title: 开发工具临时 session 生命周期回收
created_at: 2026-07-31 23
updated_at: 2026-07-31 23
decision_policy: verify_before_decision
status: implemented_pending_user_acceptance
scope:
  - scripts/node/page-debug
  - scripts/node/api-debug
  - AGENTS.md
keywords:
  - session-store
  - page-debug
  - api-debug
  - playwright
  - temporary-session
---

# 开发工具临时 session 生命周期回收

## 谁在做什么

用户确认保留产品多 session 语义，由 AI 在隔离 worktree 修复 `page-debug` 与 `api-debug` 自动化认证生命周期，并合并回 `dev`；当前等待用户人工验收。

## 为什么这样做

同一开发者的 Codex / Playwright 检查会创建多个独立 cookie context。旧工具每次登录后只关闭客户端 context，没有调用当前 session 注销接口，导致 7 天 TTL 的有效 session 在 ephemeral session-store 中累积。

## 当前方向

- 产品认证继续允许多浏览器、多设备和并行自动化 session。
- 自动化工具只删除自己刚签发的 session，不读取或删除人工浏览器 cookie。
- `page-debug` 快照、异常和 credentials-only 路径结束时回收；`open` 模式在浏览器断开时回收。
- `api-debug` 在目标请求成功或失败后都通过 `finally` 回收。
- 临时 Playwright / Node 脚本不得直接调用 sign-in；自定义流程复用临时 session owner。

## 为什么要做

让开发环境在自动化任务结束后回落到真实人工 session 数量，减少内存观察噪声和遗留有效凭证，同时不破坏生产多端登录能力。

## 截止日期

2026-07-31 已实现、集中测试并合入 `dev`；用户人工测试后完成验收。

## 验收证据

- 冻结提交：`e5595a68103d84eb94e721336af57d69aafe5120`。
- 定向 Test Batch：31/31 通过。
- 真实开发 API smoke：session-store 数量 `13 -> 13 -> 13`，`page-debug login` 与 `api-debug` 均未留下新增 session。
- 现有 13 条历史 session 未自动清理；开发 API Server 重启后重新登录可建立干净人工基线。
