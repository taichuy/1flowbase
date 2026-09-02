---
memory_type: project
topic: LLM 节点重试轮询分发方向与 Issue 1250
summary: 用户确认分发语义：none 在首次调用与所有 retry 中始终使用首个实例；retry_round_robin 才在单次调用内按 A/B/C/A 切换。分发规则插件应实现 Core 预开放的窄 RuntimeExtension slot，不因参与路由而提升为 HostExtension。
keywords:
  - retry-round-robin
  - llm-node
  - model-provider
  - distribution-rule
  - issue-1250
created_at: 2026-07-13 09
updated_at: 2026-08-28 02
last_verified_at: 2026-08-28 02
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

## 2026-08-28 架构复核与产品确认

- 用户再次确认 `none` 的正式语义：首次调用及所有 retry 都只使用首个实例；当前候选代码把 `None | RetryRoundRobin` 都映射为 `attempt_index % target_count`，属于需要后续 Delivery 修正的行为偏差。
- `retry_round_robin` 继续保持每次新 invocation 从 A 开始，仅在该 invocation 的 retry 中按 A/B/C/A 切换。
- 分发规则属于 Core 预先开放的 Provider Selection 扩展点。普通扩展实现应使用受限、隔离的 RuntimeExtension slot，而不是因“全局路由”直接升级为最高权限 HostExtension。
- HostExtension 仅用于新增底座级扩展点、替换底座机制或需要 boot/native host 能力的实现；CapabilityPlugin 继续用于工作区显式选择的应用、节点和前端能力。
- 后端 permission 决定插件可见上下文和可执行动作；typed contract、Host 结果校验、deadline 与故障隔离仍是独立于 permission 的必要边界。
- 谁在做什么：当前只完成架构判断与产品语义确认，尚未创建实施 Issue 或修改产品代码。
- 为什么要做：恢复三种分发规则的可区分语义，并让三级插件梯度真实承载不同影响面，而非把现有功能扩展一律归入 HostExtension。
- 截止日期：无固定截止日期；用户确认计划形态后再进入 Issue 与实现。

## 2026-08-28 会话来源一致性插件确认

- 用户确认验证插件放在官方插件仓 `runtime-extensions/@taichuy/session-retry-distribution`，不放入 `capability-plugins`。
- 用户确认历史绑定的 Provider 实例不可用时允许 fail closed，不静默切换其他来源。
- 用户提出将插件持久化提升为“受管插件数据模型”继续讨论：插件通过声明式 schema 申请创建自有表或向明确开放的表增加字段；不允许修改/删除物理字段，也不允许任意运行时 DDL。
- 该持久化方向尚未最终批准；需继续固定数据源绑定、跨 owner 扩字段、typed query/write Port、migration/卸载/备份与 RuntimeExtension IPC 边界后再进入计划。

## 2026-08-28 Plugin Managed Data Model 修正

- 用户否决目标表再声明 `extension_field_slot`：已有表增加插件字段时只由插件 manifest 声明，Host 依据统一全局 additive schema policy、权限、namespace 和 ownership ledger 校验，避免两个事实来源。
- 用户要求完整接口生命周期向第三方表达发生前、提交后与最终结束后的不同状态语义；同步 Hook/Decision、durable after-commit fact、terminal outcome event 和 typed Command 必须分离。
- 当前阶段继续整理底座缺失能力和后续架构优化方案，尚未创建或修改产品 Issue、代码与 schema。
