---
memory_type: project
topic: 模型供应商计费与用户额度 Issue Tree 已批准
summary: 模型计费与用户额度 Root 已本地交付并合入 dev；厂家计费目录进一步批准并实现为按 provider_code 组织的人工源、确定性自动分页、完整兼容快照，以及主仓服务端筛选分页，Root #1752 等待用户复验。
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
updated_at: 2026-08-18 17
last_verified_at: 2026-08-18 17
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

## 2026-08-18 10 厂家目录分页与组织

- 人工源固定为 `model-pricing/@<provider_code>/<model-key>/pricing.json`；目录厂家必须与文件内 `provider_code` 一致，真实 `upstream_model_id` 以文件字段为准，不从路径反推。
- 生成器确定性维护 `index.json`、`pages/*.json`、`search-index.json`、`_maintenance/catalog-state.json`、`catalog.json` 与 `dist/catalog-seed.json`；默认页大小 100。
- `model-pricing` 继续使用独立领域 schema，不复用 Extension Catalog 的 `organization` 字段，不把厂家建模为插件。
- 主仓启动远程同步改为读取并校验分页目录；扩展中心目录 API 与前端改为厂家/模型模糊筛选和服务端分页，兼容完整快照仍用于离线启动与按稳定规则 ID 导入。
- 官方仓库和主仓改动均保留在各自工作树，尚未 commit/push；集中测试已通过，Playwright 本地 browser binary 缺失，因此页面截图未验证。

## 2026-08-18 11 特殊计费规则与标准价格

- 波峰波谷继续由物理规则的有效期、星期、时区和时段表达；特殊规则 JSON 第一版只支持标准 API USD 的输入 Token 阈值分档。
- `rating_policy_enabled + rating_policy` 已贯通 migration、领域校验、repository、API、运行时预留/结算价格快照和前端编辑/列表。
- 官方目录版本 `2026-08-18.1` 当前生成 30 条规则：6 条输入 Token 分档、10 条 DeepSeek 时段规则、1 条 `zero/any` 兜底；价格只取厂家标准 API 发布价，不录入 Coding Plan、Credits、订阅价格和未开放模型。
- 当前 dev 的 3100/7800/7801 已通过 `dev-up` 重启；认证目录/规则 API 返回 200，Playwright 使用系统 Chrome 成功取得 `/settings/model-providers/pricing` 页面证据。

## 2026-08-18 13 请求日志费用快照

- `model_provider_request_logs` 按 provider attempt 保存 `pricing_provider_code`、`pricing_model_id`、`total_cost` 和 `currency_code` 不可变快照；不关联费率表外键，不在列表读取时 Join 或重算。
- 四字段必须同时为空或同时有效；历史日志保持空值，当前计费成功的 attempt 使用本地 Token 定价结果，金额继续以 Decimal 字符串进入 API。
- `/settings/model-providers/request-logs` 默认展示费用列，并提供供应商 Code、计费模型 ID 两个可选列；USD 继续使用数字在前、`$` 在后的金额格式。
- dev migration、request-log worker、repository、API、runtime metadata 恢复、前端定向测试和受保护页面运行态均已验证；等待用户人工触发一次新模型请求复验真实非空费用行。

## 2026-08-18 17 多轮模型调用计费与日志一致性修复

- 每次真实上游模型调用生成独立 `provider_invocation_id`，内部工具回调前后的多轮调用不再复用 `billing_session_id`；`runtime_cost_ledger_billing_session_uidx` 继续保留。
- LLM 循环累积每轮 provider attempt 与 Token usage，全局重新编号 attempt；工具回调本身不伪造模型请求。
- usage、cost、credit settlement、billing session 状态和 outbox 收敛到同一 PostgreSQL 事务；本地计费结算失败保留上游正常输出，通过 `billing_status=reconciliation_failed` 单独暴露，不再改写为 `provider_invalid_response`。
- request log 按每个 attempt 持久化，增加 `billing_status`，并在没有 external conversation 时回填 `flow_runs.assistant_conversation_id`。
- 定向 Rust/PostgreSQL/API/前端测试、`cargo fmt --check`、`git diff --check`、migration、后端重启、health 和认证 request-log API 已通过；历史异常 run 保持原始账本，不做静默追溯修改。
