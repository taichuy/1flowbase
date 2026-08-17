## 现状

1flowbase 已经具备部分计费基础：

- `runtime_usage_ledger`：记录模型调用的输入、输出、缓存等 token。
- `runtime_cost_ledger`：记录成本、币种和价格快照。
- `runtime_credit_ledger`：记录额度变动流水。
- `billing_sessions`：记录预留、结算、退款状态。
- `model_provider_request_logs`：记录用户、供应商、模型、请求尝试和 token。
- `storage-ephemeral`：已有缓存、租约、分布式锁、事件总线和任务队列。

当前缺失的是完整闭环：

- 没有厂家计费规则表和官方计费目录。
- 没有真实费率匹配。
- 没有用户额度账户投影。
- 没有模型调用前的原子额度预留。
- 没有请求结束后的真实 token 结算。
- 没有面向插件开放的额度命令和事件。
- 用户管理、角色权限和账单查询尚未接入。

已确认的产品决策：

- 第一阶段只使用 `USD`，API/数据库存 `USD`，UI 展示 `$`。
- 额度按 `workspace_id + user_id` 隔离。
- root 默认不扣费。
- 余额为 `0` 时允许一次边界请求，结算后可以为负数。
- 计费挂在模型供应商宿主调用边界，覆盖所有模型请求入口。
- 历史 usage 按 USD 0 处理；计费启用后的模型没有规则则拒绝调用。
- 官方规则按稳定 ID 新增或更新，不因源端缺失删除本地数据。
- 插件可以申请改变额度、监听结果，但不能直接修改余额。

## 需求分析

### 一、总体架构

采用成熟的 Metering → Rating → Ledger → Balance Projection：

```text
模型调用方
    │
    ▼
Host ModelProvider Invocation
    │
    ├── 1. 匹配计费规则
    ├── 2. 检查并预留额度
    ├── 3. 调用 Provider RuntimeExtension
    ├── 4. 取得实际 token usage
    ├── 5. 按本地规则计算 USD
    ├── 6. 写 usage / cost / credit ledger
    └── 7. 结算余额并发布事件
```

所有入口统一经过这一层：

```text
Agent Flow ───────┐
Workflow ─────────┤
Assistant ────────┤
公开应用 API ─────┤
Capability Plugin ┤
未来其他入口 ─────┘
                  ▼
       ModelProvider Invocation
```

这样不会在 Agent Flow、Workflow、公开 API 中分别实现计费。

### 二、真值与 owner

| 对象 | Owner | Source of truth |
|---|---|---|
| 官方厂家计费模板 | 官方插件仓库 | 签名 catalog |
| 本地实际采用的费率 | 主数据库 | `model_pricing_rules` |
| 模型使用量 | 模型供应商调用边界 | usage ledger |
| 某次消费采用的价格 | cost ledger | `price_snapshot` |
| 额度变动原因 | credit ledger | append-only ledger |
| 当前余额 | credit account | durable 热投影 |
| 费率查询缓存 | ephemeral | 可丢失加速层 |

官方仓库是模板真值；导入本地后，本地数据库是运行时真值。历史消费由 `price_snapshot` 冻结，不随规则更新重新计算。

---

### 三、厂家计费规则表

第一阶段采用单表，不关联供应商实例、插件安装记录或模型定义外键：

```text
model_pricing_rules
├── id
├── provider_code
├── upstream_model_id
│
├── input_token_unit_size
├── input_token_unit_price
├── output_token_unit_size
├── output_token_unit_price
├── cache_hit_token_unit_size
├── cache_hit_token_unit_price
│
├── currency_code
├── effective_from
├── effective_to
├── timezone
├── weekday_mask
├── local_time_start
├── local_time_end
├── priority
├── enabled
│
├── source_kind
├── source_catalog_id
├── source_version
├── source_checksum
├── extensions
│
├── created_by
├── created_at
└── updated_at
```

核心六个费率字段：

```text
input_token_unit_size
input_token_unit_price

output_token_unit_size
output_token_unit_price

cache_hit_token_unit_size
cache_hit_token_unit_price
```

示例：

```text
input_token_unit_size       = 1000000
input_token_unit_price      = 1.25

output_token_unit_size      = 1000000
output_token_unit_price     = 5.00

cache_hit_token_unit_size   = 1000000
cache_hit_token_unit_price  = 0.25
```

字段类型：

```text
*_unit_size   bigint
*_unit_price  numeric(38, 18)
currency_code text not null default 'USD'
extensions    jsonb not null default '{}'
```

约束：

```text
unit_size > 0
unit_price >= 0
currency_code = 'USD'       # 第一阶段
effective_to > effective_from
priority >= 0
```

六个计费字段全部 `NOT NULL`：

- `0` 表示明确免费。
- 不使用 `NULL` 表示未配置。
- 没有匹配规则和匹配到零价格规则是两种不同状态。

`extensions` 不参与第一阶段核心计算，只保存未来特殊计费维度，例如 reasoning token、cache write 或厂家说明信息。稳定后再通过 migration 上提为正式列。

### 四、计费规则时间表达

一条规则同时表达：

1. 版本有效期。
2. 周期性波峰波谷时段。

```text
effective_from / effective_to
    → 这版价格在哪个日期范围有效

timezone + weekday_mask + local_time_start/local_time_end
    → 每周哪些日子的哪个时段有效
```

时间区间统一采用半开区间：

```text
[effective_from, effective_to)
[local_time_start, local_time_end)
```

跨午夜时段第一阶段不直接允许：

```text
22:00–02:00
```

应拆成：

```text
22:00–24:00
00:00–02:00
```

避免星期归属和夏令时产生歧义。

规则匹配顺序：

```text
1. provider_code 精确匹配
2. upstream_model_id 精确匹配
3. request_started_at 命中有效期
4. 换算到规则 timezone 后命中星期和时段
5. 取最高 priority
6. 同优先级仍有多条则报 pricing_rule_conflict
```

同一 `provider_code + upstream_model_id + 时间窗口 + priority` 不允许重叠。

### 五、token 计费算法

计费只依据 token，不读取厂家余额、厂家账单或上游实际扣款金额。

标准数量：

```text
cache_hit_tokens =
  input_cache_hit_tokens

ordinary_input_tokens =
  input_cache_miss_tokens
  或 max(input_tokens - cache_hit_tokens, 0)

billable_output_tokens =
  output_tokens
```

计算：

```text
input_cost =
  ordinary_input_tokens
  × input_token_unit_price
  ÷ input_token_unit_size

output_cost =
  billable_output_tokens
  × output_token_unit_price
  ÷ output_token_unit_size

cache_hit_cost =
  cache_hit_tokens
  × cache_hit_token_unit_price
  ÷ cache_hit_token_unit_size

total_cost =
  input_cost + output_cost + cache_hit_cost
```

中间过程使用 decimal，不使用浮点数。各分项不提前舍入，合计后以 `numeric(38,18)` 保存。

cost ledger 保存完整快照：

```json
{
  "pricing_rule_id": "rule-id",
  "provider_code": "openai",
  "upstream_model_id": "gpt-x",
  "currency_code": "USD",
  "request_started_at": "...",
  "input_token_unit_size": 1000000,
  "input_token_unit_price": "1.25",
  "output_token_unit_size": 1000000,
  "output_token_unit_price": "5.00",
  "cache_hit_token_unit_size": 1000000,
  "cache_hit_token_unit_price": "0.25",
  "ordinary_input_tokens": 100000,
  "output_tokens": 50000,
  "cache_hit_tokens": 200000
}
```

后续修改或删除计费规则不会改变历史金额。

### 六、usage 来源与重试

模型供应商 contract 应返回 token usage。

usage 来源记录为：

```text
provider_reported
host_counted
```

处理顺序：

1. 优先使用厂家响应中的 usage。
2. 厂家没有返回时，使用供应商插件已有 token counter。
3. 两者都不可用时，视为 provider metering contract 异常，不静默按零消费。

重试和故障转移必须按每次上游调用分别记录：

```text
Provider Attempt 1 → usage + cost
Provider Attempt 2 → usage + cost
Provider Attempt 3 → usage + cost
```

最终请求成本是所有实际尝试成本之和，不能只结算成功的最后一次。

因此需要把当前 usage ledger 收敛为“每个 provider attempt 一条 usage”，通过现有 `failover_attempt_id` 或等价请求尝试 ID 关联。

### 七、历史零计费和当前规则缺失

设置计费启用时间：

```text
billing_enabled_at = T0
```

历史请求：

```text
request_started_at < T0
    → pricing_match_status = historical_zero
    → normalized_cost = 0
    → settlement_currency = USD
    → 不追溯扣费
```

历史价格快照：

```json
{
  "rule_kind": "historical_zero",
  "billing_enabled_at": "...",
  "reason": "usage_before_billing_activation"
}
```

当前请求：

```text
request_started_at >= T0
    → 必须命中计费规则
    → 未命中则在调用厂家前拒绝
```

错误：

```text
pricing_rule_not_configured
```

处理：

- 不调用上游模型。
- 不产生 token。
- 不扣费。
- 记录 provider/model 和用户上下文。
- UI 提示管理员配置或导入厂家计费规则。

即使 root 不扣额度，当前模型也必须具备计费规则，以保证成本统计完整。

### 八、官方计费目录

官方插件仓库新增独立目录，例如：

```text
model-pricing/
├── catalog/v1/index.json
├── releases/v1/catalog.json
├── @taichuy/openai/pricing.json
├── @taichuy/deepseek/pricing.json
└── schemas/
```

不把价格绑定到供应商 Rust 二进制版本。价格目录独立签名、校验 checksum 和发布版本。

#### 开发环境

```text
数据库无官方计费规则
    → 拉取官方 catalog
    → 校验签名/checksum
    → 按稳定 ID Upsert
```

#### 打包环境

```text
应用包内置 catalog snapshot
    → 启动时导入/更新
    → 网络可用时检查新版本
```

#### 导入规则

```text
ID 已存在   → 更新
ID 不存在   → 新增
源端未出现  → 不删除本地记录
```

用户编辑官方导入规则后，如果下次同步仍命中相同 ID，会被官方值覆盖。

用户希望永久保留修改时，应执行“复制为自定义规则”，生成新的本地 ID：

```text
source_kind = manual
source_catalog_id = null
```

自定义规则不会被官方同步修改或删除。

### 九、扩展中心与模型供应商 UI

#### 扩展中心

新增“厂家模型计费”Tab：

```text
扩展中心
├── 已安装
├── Runtime Extensions
├── MCP
├── Agent Flow
└── 厂家模型计费
```

功能：

- 浏览官方厂家计费目录。
- 查看版本和 checksum。
- 导入或更新规则。
- 查看新增、更新数量。
- 不执行远端缺失删除。
- 查看本地自定义规则数量。

#### 模型供应商设置

在 `/settings/model-providers/providers` 中：

- 模型列表展示“计费已配置/未配置”。
- 操作列增加“计费规则”。
- 新增或编辑规则时选择：

  - `provider_code`
  - `upstream_model_id`
  - 输入价格
  - 输出价格
  - 缓存命中价格
  - 生效日期
  - 周期时段
  - 时区
  - 优先级

- 配置冲突时阻止保存。
- 未配置规则的模型明确显示不可运行原因。

计费规则表注册成内置数据模型并生成 CRUD，但正式设置页面仍经过后端校验服务，不能由前端承担冲突判断。

### 十、用户额度账户

新增：

```text
user_credit_accounts
├── id
├── workspace_id
├── user_id
├── credit_unit
├── charge_enabled
├── current_balance
├── reserved_amount
├── revision
├── created_at
└── updated_at
```

约束：

```text
unique(workspace_id, user_id, credit_unit)
credit_unit = 'USD'
reserved_amount >= 0
```

含义：

```text
available_balance =
  current_balance - reserved_amount
```

root 默认：

```text
charge_enabled = false
```

root 请求仍产生 usage 和 cost ledger，但不从 credit account 扣款，结算记录标记：

```text
charge_skipped = true
charge_skip_reason = root_exempt
```

### 十一、用户管理页面

用户管理后台增加 Tab：

```text
用户管理
├── 用户
└── 用户金额
```

列表字段：

- workspace。
- 用户账号和名称。
- 是否扣费。
- 当前余额。
- 已预留额度。
- 可用额度。
- 最后更新时间。

操作：

- 增加额度。
- 直接扣除。
- 调整余额。
- 启用/关闭扣费。
- 退款。
- 查看额度流水。

用户资料接口不包含可编辑金额字段。

### 十二、额度流水

复用并补强现有 `runtime_credit_ledger`，不再创建重复的“用户金额流水表”。

建议结构：

```text
runtime_credit_ledger
├── id
├── transaction_id
├── account_id
├── workspace_id
├── user_id
├── application_id
├── flow_run_id
├── span_id
├── billing_session_id
├── cost_ledger_id
├── actor_user_id
├── actor_plugin_id
├── transaction_type
├── amount
├── balance_after
├── reserved_after
├── credit_unit
├── reason
├── source_type
├── source_id
├── idempotency_key
├── status
├── metadata
└── created_at
```

流水类型：

| 类型 | amount | 作用 |
|---|---:|---|
| `grant` | 正数 | 增加额度 |
| `charge` | 负数 | 直接扣除 |
| `reserve` | 0 | 增加预留 |
| `settle` | 负数 | 按 token 成本结算 |
| `release` | 0 | 释放未使用预留 |
| `refund` | 正数 | 退款 |
| `adjustment` | 正或负 | 管理员调整 |

账本规则：

- Append-only。
- 不更新或删除历史流水。
- 错误通过反向流水纠正。
- 账户投影和账本流水在同一事务提交。
- `idempotency_key` 防止重复入账。

索引：

```text
(account_id, created_at desc, id desc)
(workspace_id, user_id, created_at desc, id desc)
(billing_session_id)
(flow_run_id)
unique(account_id, idempotency_key)
```

流水页面使用 keyset pagination，不通过实时 `SUM()` 推导余额。

### 十三、额度检查与原子预留

准入规则：

```text
charge_enabled = false
    → 允许

charge_enabled = true
且 available_balance >= 0
    → 允许并预留

charge_enabled = true
且 available_balance < 0
    → 拒绝
```

余额为 `0` 时允许一个边界请求。

预留事务：

```sql
begin;

select *
from user_credit_accounts
where workspace_id = ?
  and user_id = ?
  and credit_unit = 'USD'
for update;

-- 检查 available_balance
-- 创建 billing_session
-- 写 reserve ledger
-- 增加 reserved_amount

commit;
```

模型网络请求不能放在数据库事务里。完整过程是两个短事务：

```text
事务 A：检查和预留
    ↓
事务外：调用模型
    ↓
事务 B：结算和释放
```

预留金额根据：

- 当前输入 token 数量。
- 请求最大输出 token。
- 命中的计费规则。
- 必要时的最小预留金额。

如果规则所有价格都是 `0`，预留金额可以是 `0`。

### 十四、结算状态机

复用 `billing_sessions`：

```text
reserved
├── settled
├── refunded
└── failed
```

正常结算：

```text
actual_cost < reserved
    → 扣 actual
    → 释放差额

actual_cost = reserved
    → 完整结算

actual_cost > reserved
    → 扣 actual
    → 余额允许变负
```

模型请求失败：

- 有 token usage：仍按 token 结算。
- 无 token usage：释放预留。
- usage contract 异常：记录 billing failure 和高优先级日志，进入补偿处理。

幂等键应基于稳定调用身份，例如：

```text
reserve:{provider_invocation_id}
settle:{provider_invocation_id}
release:{provider_invocation_id}
refund:{original_ledger_id}
```

重复回调、任务重试和进程恢复不能重复扣款。

### 十五、超时 reservation 回收

durable `billing_sessions` 增加：

```text
reservation_expires_at
last_heartbeat_at
```

长时间流式请求定期 heartbeat。

回收任务：

```text
查询超时 reserved session
    → FOR UPDATE SKIP LOCKED
    → 检查调用租约是否仍存活
    → 已死亡则 release
    → 写 ledger 和事件
```

ephemeral lease 只协助判断执行 owner 是否存活，不能仅凭缓存 TTL 直接退款。

### 十六、插件额度命令

插件不获得数据表写权限，而是向 Core 发送结构化命令：

- `GrantCredit`
- `ChargeCredit`
- `ReserveCredit`
- `SettleCredit`
- `ReleaseCredit`
- `RefundCredit`

示例：

```json
{
  "command": "grant_credit",
  "workspace_id": "workspace-1",
  "user_id": "user-1",
  "amount": "2.50",
  "credit_unit": "USD",
  "reason": "daily_checkin",
  "source_type": "checkin",
  "source_id": "2026-08-17",
  "idempotency_key": "checkin:user-1:2026-08-17"
}
```

Core 负责：

- 校验插件 capability permission。
- 校验 workspace 和用户。
- 校验金额与币种。
- 校验命令类型。
- 检查幂等键。
- 锁定额度账户。
- 写 ledger。
- 更新账户投影。
- 提交后发布结果事件。

### 十七、额度事件

成功或拒绝后发布：

- `CreditGranted`
- `CreditCharged`
- `CreditReserved`
- `CreditSettled`
- `CreditReleased`
- `CreditRefunded`
- `CreditCommandRejected`

使用 transactional outbox 保证数据库提交与事件发布之间不丢失。

事件采用 at-least-once 投递，插件消费者必须按 `event_id` 幂等。

插件可以用事件：

- 发送到账通知。
- 更新支付订单。
- 解锁付费功能。
- 记录业务统计。
- 生成发票或对账记录。
- 调用外部 webhook。

最终边界：

```text
插件拥有业务规则
Core 拥有账户、权限、幂等、事务和账本
插件通过命令申请变更
Core 通过事件公布结果
```

### 十八、角色和接口权限

用户金额操作必须注册到角色管理，且保持：

```text
1 operation ↔ 1 HTTP method + route template
```

建议拆分为独立 operation：

- 查看用户额度列表。
- 查看单个用户额度。
- 查看额度流水。
- 增加额度。
- 直接扣除。
- 调整额度。
- 启用扣费。
- 关闭扣费。
- 退款。
- 管理厂家计费规则。
- 导入官方计费规则。

root/system admin 默认拥有；其他角色从角色管理页面显式授权。

普通用户不能通过：

- 用户资料更新接口。
- Runtime CRUD。
- 数据模型通用写接口。
- 插件 SQL。
- 前端表单伪造。

修改自己的额度。

若开放“查看自己的余额和账单”，应使用单独的 self-read operation，并由后端限定 `actor.user_id`。

### 十九、Ephemeral 层

#### 适合缓存

费率规则编译为：

```text
HashMap<
  (provider_code, upstream_model_id),
  SortedIntervalRules
>
```

查找过程：

- HashMap 定位厂家和模型：平均 `O(1)`。
- 有效期规则二分查找：`O(log n)`。
- 当前周期窗口局部筛选。

缓存过期：

```text
expires_at =
  min(
    configured_ttl,
    next_effective_boundary,
    next_peak_window_boundary
  )
```

规则导入、编辑、启停后发布 revision invalidation event，主动清理缓存。

#### 不适合缓存

以下对象不得以 ephemeral 为真值：

- 当前余额。
- 已预留额度。
- billing session。
- credit ledger。
- idempotency key。
- 历史价格快照。

第一阶段额度准入直接使用 PostgreSQL 原子事务。相对于模型网络请求，这一数据库开销很小，但能避免 Redis 和数据库双写一致性问题。

### 二十、日志与可观测性

每次 provider attempt 至少记录：

- workspace/user/application。
- provider code。
- upstream model ID。
- pricing rule ID。
- pricing match status。
- usage source。
- 输入 token。
- 输出 token。
- 缓存命中 token。
- 分项成本。
- 总成本。
- reservation ID。
- billing session ID。
- settlement status。
- 是否跳过扣费及原因。
- idempotency key。

重要状态：

```text
pricing_rule_matched
pricing_rule_not_configured
pricing_rule_conflict
historical_zero
provider_reported
host_counted
credit_reserved
credit_settled
credit_released
credit_insufficient
billing_failed
root_exempt
```

### 二十一、验收标准

1. 所有模型调用入口统一经过计费门面。
2. 计费规则使用单表和六个固定费率字段。
3. 核心金额计算不依赖 JSONB。
4. 历史 usage 结算为 USD 0，不追溯扣费。
5. 当前模型没有计费规则时，在调用厂家前拒绝。
6. 相同 usage、规则和时间点始终得到相同金额。
7. 缓存命中 token 不与普通输入 token 重复扣费。
8. 重试和故障转移中的每次上游请求分别计费。
9. 历史金额不受规则更新影响。
10. 余额为零时，并发请求最多一个跨过边界。
11. root 默认不扣费，但仍记录成本。
12. 所有额度变化都有 append-only ledger。
13. 命令重试不会重复加款、扣款或退款。
14. 用户不能修改自己的额度。
15. 角色管理可分别授权额度操作。
16. 官方同步只新增或按 ID 更新，不删除本地规则。
17. ephemeral 全部丢失后，账户和账单仍可恢复。
18. 超时 reservation 可安全回收。
19. 插件只能通过 Credit Command 改变额度。
20. Credit Event 重复投递不会导致插件重复处理。

## 三个方向（升温发散）

### 保守

- 方案内容：只实现计费规则、成本统计和事后扣款，不做原子预留、插件命令和完整账户状态。
- 综合收益：实施较快，但无法保证并发额度边界，后续签到、支付和业务收费还需要重构。

### 平衡

- 方案内容：复用已有 usage/cost/credit/billing 基础，增加单表计费规则、用户额度账户、模型供应商统一计费门面、原子预留结算、用户金额管理、角色权限及 Credit Command/Event。
- 综合收益：满足当前完整目标，把厂家数据留在官方插件仓库，把资金一致性留在 Core，复杂度与可扩展性匹配。

### 激进

- 方案内容：同时引入多币种、汇率、支付订单、税费、发票、完整双分录会计和分布式额度中心。
- 综合收益：可以发展成商业 Billing 平台，但明显超出当前 USD 模型额度目标。

## 最终建议（降温收敛）

采用平衡方向，正式架构口径为：

```text
官方插件仓库
    → 厂家计费模板和签名 catalog

主仓 Core
    → 规则导入
    → token 计量
    → USD 定价
    → 原子预留
    → 账本结算
    → 权限和幂等
    → Credit Command/Event

用户管理
    → 用户金额账户和额度流水

Ephemeral
    → 费率缓存、租约、任务和事件加速
    → 不承载余额真值
```

这是跨主仓、官方插件仓库、模型供应商运行时、用户管理、权限和事件基础设施的长计划，后续应使用两层 Issue Tree：

```text
Root：模型计费与用户额度闭环
├── Delivery 1：官方厂家计费 catalog 与导入同步
├── Delivery 2：计费规则数据模型、匹配算法与缓存
├── Delivery 3：用户额度账户、账本和权限
├── Delivery 4：模型供应商统一预留、计量与结算
├── Delivery 5：Credit Command / Event 扩展边界
└── Delivery 6：前端管理页面与集中 QA
```

当前停止条件是你确认这份完整方案；确认后再读取长计划规则，形成正式 Root/Delivery Issue Tree，不在本轮直接修改产品代码。