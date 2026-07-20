---
memory_type: project
topic: AI Gateway 手动并发门禁已批准并进入集中验收
summary: 用户批准 Root #1377 的平衡方案及证据拓扑补强：使用独立 workflow_dispatch、确定性 Mock SSE/WebSocket 上游、临时发布的 Agent Flow 应用、合成并发负载与隔离 Codex/Claude 哨兵，在 GitHub 托管 runner 首次 characterize；不改公共 WebSocket、Provider 单飞、数据库默认 pool 或持久化 owner。
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
updated_at: 2026-07-20 09
last_verified_at: 2026-07-20 09
decision_policy: verify_before_decision
scope:
  - .github/workflows/ai-gateway-concurrency.yml
  - scripts/node/ai-gateway-concurrency
  - https://github.com/taichuy/1flowbase/issues/1377
---

# AI Gateway 手动并发门禁已批准并进入集中验收

## 谁在做什么

Root agent 按 #1377 调度三项 Delivery：#1378 的 Mock/负载证据引擎、#1379 的真实网关 fixture 与隔离 CLI 哨兵、#1380 的手动 workflow 与首次 characterize。

实现中的 Application 统一指临时发布的 Agent Flow 应用。Anthropic 同池请求复用一个临时 Agent Flow 应用；多池请求同时分发到两个临时 Agent Flow 应用，各自绑定独立 Provider instance 和 Application API key。

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

## 集中 QA 暴露的证据拓扑决策

- QA cycle 1 发现 Node `Array.map(path.basename)` 回调签名错误，修复后定向测试恢复。
- QA cycle 2 的机械 Batch 全绿，但发现原 runner 未消费 durable/runtime activity/active streams、没有 Anthropic multi-pool 曲线、成功路径不保存服务日志，因此禁止 merge/push/dispatch。
- 用户再次批准平衡方向：只补门禁内部证据拓扑，不降低 Root AC。
- Durable ledger 复用现有 `metadata.trace_id`、run list/query、runtime activity 与 plugin active-stream owner 接口；不新增 API DTO。
- Multi-pool 只把两个现有 `endpoint + Application API key + published model` tuple 放入 workflow-private topology collection，不新增 workflow input 或第四 tuple 字段。
- api-server/plugin-runner 日志必须在 cleanup 前以固定上限、脱敏形式写入 governance artifact；写入失败仍完成进程与 scratch cleanup，并使门禁失败。
