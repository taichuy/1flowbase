---
memory_type: project
topic: 模型供应商计费与用户额度 Issue Tree 已批准
summary: 模型计费与用户额度 Root 已本地交付并合入 dev；人工验收反馈进一步收敛为独立厂家计费目录、单条 zero/any 全局零价兜底、精确规则优先及 K/M/B 与有效小数展示，Root #1752 等待用户复验。
keywords:
  - model-pricing
  - user-credit
  - billing-session
  - token-cost
  - credit-command
  - credit-event
  - issue-1752
match_when:
  - 开始或继续模型计费与用户额度实现
  - 修改模型供应商 usage/cost/credit ledger
  - 增加厂家计费目录或用户金额后台
  - 开放插件额度命令与事件
created_at: 2026-08-17 12
updated_at: 2026-08-17 20
last_verified_at: 2026-08-17 20
decision_policy: verify_before_decision
status: user_acceptance
scope:
  - https://github.com/taichuy/1flowbase/issues/1752
  - https://github.com/taichuy/1flowbase/issues/1754
  - https://github.com/taichuy/1flowbase/issues/1755
  - https://github.com/taichuy/1flowbase/issues/1753
  - https://github.com/taichuy/1flowbase/issues/1756
  - /home/taichuy/git/1flowbase
  - /home/taichuy/git/1flowbase-official-plugins
---

# 模型计费与用户额度 Root

## 谁在做什么

- Root #1752 是计划、进度、集中 QA 和用户最终验收的唯一线上真值。
- #1754 交付厂家目录、单表费率和运行时规则生效。
- #1755 交付 workspace 用户额度、append-only 流水与角色治理。
- #1753 交付全模型调用原子预留、逐上游尝试 Token 定价与可恢复结算。
- #1756 交付插件 Credit Command、capability permission 与可靠 Credit Event。

## 为什么这样做

- 现有 `runtime_usage_ledger`、`runtime_cost_ledger`、`runtime_credit_ledger` 和 `billing_sessions` 已有骨架，但只存在 0 金额预留，尚无真实规则、账户和结算闭环。
- 厂家模板由官方插件仓库维护，本地数据库拥有运行时规则真值；Core 独占账户、权限、幂等、事务和账本。

## 已冻结边界

- 第一阶段只支持 USD；额度按 `workspace + user` 隔离，root 默认不扣费。
- 费率单表保存输入/输出/缓存命中三组 `unit_size + unit_price` 六列，JSONB 只作未来扩展。
- 历史 usage 为 `historical_zero`；计费启用后没有规则则在调用上游前拒绝。
- 全部模型入口在 Host ModelProvider Invocation 统一计费，重试/故障转移逐 provider attempt 计算 Token 成本。
- 插件只能通过结构化命令申请额度变更并监听可靠事件，不得直写余额或 ledger。

## 执行停止条件

- 未完成唯一只读 Scout、有限 Work Packet inventory 与 Root 集中 Test Batch 前不进入开发。
- 新增多币种、支付/税务、厂家账单 reconciliation、公式 DSL、历史追溯扣款或插件直写余额时返回 problem-framing。

## 2026-08-17 20 验收反馈修复

- 厂家计费目录独立于 RuntimeExtension / 插件模型清单；相邻官方仓库只承载 catalog 发布真值。
- 官方默认目录只保留 `provider_code=zero`、`upstream_model_id=any` 的单条零价兜底。
- 匹配顺序为精确 `provider/model` 优先于 `zero/any`；删除或停用兜底后恢复缺规则拒绝。
- 前端 Token 单位显示 `K/M/B`；零价显示 `$0`，非零金额去除数据库无意义尾零并至少展示两位常规小数。
- beta 提交 `7f7669fea` 与 `9ed6d1bae`，最终本地 dev merge `555dee792`，官方目录提交 `56815aa`；均未 push。
- beta 数据库 migration、桌面/移动页面、前端、control-plane、storage-postgres 和 API 精确模块测试均已通过，证据已回填 Root #1752。
- 集中 API 验证发现旧 Gateway 账本写入遗漏新 NOT NULL `transaction_id`；已按 migration 历史语义修复为 `transaction_id = ledger_id`，repository 1/1、beta Application Runtime 6/6、dev 合并态 6/6 通过。
