---
memory_type: project
topic: LLM 节点重试轮询分发方向与 Issue 1250
summary: 用户确认并授权实现 retry_round_robin：每轮首次请求固定 target A，仅该轮 LLM 节点重试按 A/B/C/A 轮询，下一轮重置为 A；Issue #1250 已完成实现与定向 QA，等待用户验收。
keywords:
  - retry-round-robin
  - llm-node
  - model-provider
  - distribution-rule
  - issue-1250
created_at: 2026-07-13 09
updated_at: 2026-07-13 09
last_verified_at: 2026-07-13 09
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1250
  - api/crates/orchestration-runtime
  - api/crates/domain/src/model_provider.rs
  - api/crates/storage-durable/postgres
  - web/app/src/features/settings
---

# LLM 节点重试轮询方向

## 谁在做什么

用户已确认 Issue #1250 并授权直接实现。AI 已完成 `retry_round_robin` 的领域、API、持久化 migration、编译、执行、前端设置和多语言 contract，当前等待用户验收。

## 为什么这样做

现有 `none` 会让 LLM 节点重试继续命中原实例；现有 `round_robin` 为每个 attempt 消费共享计数，并发交错时不能保证某次重试确定进入本次调用的下一个实例。新规则需要把失败切换限定在单次 LLM 节点调用内部。

## 已确认决策

- 每轮 attempt 0 固定选择 target A。
- 只有 LLM 节点已经决定执行的 retry 才按 B、C、A 继续轮询。
- 下一轮新的 LLM 节点调用重置，从 A 开始，不继承上一轮位置。
- 重试次数仍由 LLM 节点 `max_retries` 控制；实例数不足时循环。
- 不改变 `none`、`round_robin` 与首 token 后失败不切换的既有行为。
- 新规则不消费共享 round-robin counter，也不向 provider invocation 或直接调用方扩散 retry flag。

## 为什么要做

产品上区分“跨请求负载均衡”和“单次请求失败切换”，工程上让重试顺序不受并发共享状态干扰，同时保持调用方 contract 简单。

## 实现与验证状态

- runtime 定向测试覆盖每轮 A 重置、A/B/C/A 循环、并发请求隔离、首 token 后停止和既有 `none / round_robin` 回归。
- console route 测试覆盖 `retry_round_robin` 保存、响应与 options 回读。
- 实际 migration 文件已在本地 PostgreSQL 独立 schema 执行，三种合法值通过、非法值被 check constraint 拒绝，测试 schema 已删除。
- 设置页选项、mutation、共享 settings/LLM contract consumer 测试通过；i18n hygiene 为 0 error，新增 key 无 finding。
- `storage-postgres` 全 crate test 被仓库既有 Workflow trigger/request-log fixture 编译错误阻断；共享 contract gate 37 项中 36 项通过，唯一失败为与本任务无关的 MCP 目录编辑场景。

## 截止日期与下一步

无固定截止日期。当前进入用户验收；后续如需把首次请求也加入全局轮询，必须另行引入双策略设计，不在本 Issue 内隐式扩展。
