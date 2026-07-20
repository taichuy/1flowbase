---
memory_type: project
topic: AI Gateway 手动并发门禁已批准并进入实现
summary: 用户批准 Root #1377 的平衡方案：使用独立 workflow_dispatch、确定性 Mock SSE/WebSocket 上游、合成并发负载与隔离 Codex/Claude 哨兵，在 GitHub 托管 runner 首次 characterize；不改公共 WebSocket、Provider 单飞、数据库默认 pool 或持久化 owner。
keywords:
  - ai-gateway
  - concurrency
  - github-actions
  - workflow-dispatch
  - codex
  - claude-code
  - mock-upstream
  - characterize
match_when:
  - 继续执行或验收 GitHub Issue #1377
  - 讨论 AI Gateway 并发、Mock 上游或真实 CLI 门禁
  - 判断并发验证应在本机还是 GitHub Actions 运行
created_at: 2026-07-19 18
updated_at: 2026-07-19 18
last_verified_at: 2026-07-19 18
decision_policy: verify_before_decision
scope:
  - .github/workflows/ai-gateway-concurrency.yml
  - scripts/node/ai-gateway-concurrency
  - https://github.com/taichuy/1flowbase/issues/1377
---

# AI Gateway 手动并发门禁已批准并进入实现

## 谁在做什么

Root agent 按 #1377 调度三项 Delivery：#1378 的 Mock/负载证据引擎、#1379 的真实网关 fixture 与隔离 CLI 哨兵、#1380 的手动 workflow 与首次 characterize。

## 为什么这样做

用户希望评估 #1366 重构后的 AI Gateway 并发行为，但没有大量真实 API key，也不能让本机正在使用的 Codex/Claude Code 因配置或资源争抢中断。并发验证因此迁移到仅手动触发的 GitHub Actions，模型上游使用 loopback Mock。

## 为什么要做

当前 Provider contract 明确存在 same-pool 单飞与 OpenAI stateful worker 串行语义。需要把“请求都成功”“同池排队”“多池真实并行”拆开观测，并验证流式终态、取消、失败和 durable terminal 一致性。

## 截止日期

未设置时间截止；Done 只由 #1377 AC 与集中 QA/首次 characterize 证据决定。

## 决策背后动机

- 独立 `workflow_dispatch`，不进入 schedule、默认 CI 或每次 PR。
- Codex/Claude 只是真实客户端兼容哨兵，主并发由轻量合成客户端产生。
- 首次 `characterize` 从正确性、唯一终态和并发 contract 开始硬门禁；绝对性能预算等待首轮证据和用户确认。
- GitHub 托管 runner 只代表固定 CI 回归环境，不宣称生产容量。
- 不读取本机 `.memory`、`.env` 或全局 CLI 配置；不使用真实 Provider secret。
- 新公共 Responses WebSocket、Provider 单飞、数据库默认 pool、schema、权限或持久化 owner 变化必须回到需求对齐。
