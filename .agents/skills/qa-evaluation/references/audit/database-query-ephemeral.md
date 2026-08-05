# Database, Query, And Ephemeral Audit

## Goal

区分 schema 静态正确性、真实 catalog/capacity 和 query plan/runtime 三层证据；审计索引、增长、JSONB、日志查询、retention 与 ephemeral 放置，不把规划信号冒充性能回归。

## Invariants

```text
BoundedQuery = Scope ∧ Time ∧ Cursor ∧ Limit ∧ StableOrder

Ephemeral(x) =
  DerivedOrRebuildable
  ∧ LossTolerant
  ∧ TTLBounded
  ∧ CapacityBounded
  ∧ Observable
  ∧ InvalidationOwned
  ∧ NotSourceOfTruth
```

- Durable 业务事实、审计结果和状态机终态不得只存在于 cache/stream。
- cache key 必须覆盖 scope、权限或策略版本、查询语义与必要 generation。
- list/overview 优先 summary/preview；raw JSONB 只由主键、run scope 或 detail 入口读取。
- migration、约束、scope owner chain 与历史 checksum 是 schema blocker 边界。
- 索引存在不推出查询使用索引；表大不推出必须分区。

## Evidence

1. **Static contract**：复用 `schema-hygiene`、`growth-table-report`、`raw-jsonb-report`、`log-query-contract-report`，检查迁移、列、约束、索引形状和有界查询。
2. **Catalog/capacity**：候选绑定的 PostgreSQL schema、表/索引大小、row estimate、统计采集时间；复用 `capacity-report`。
3. **Plan/runtime**：隔离 fixture 上少量关键查询的 `EXPLAIN (FORMAT JSON)`；只有需要真实规模时才专项运行 `EXPLAIN ANALYZE` 或 load/soak。

索引建议记录查询入口、filter/order/join、现有索引列序、计划节点、估算/实际行数、写频率、存储与回滚。Ephemeral inventory 记录 owner、TTL、entry/byte capacity、payload cap、inspection、cleanup、overflow 与 durable fallback。

## Legal Negatives

- 小表顺序扫描可能优于索引扫描；不能把 `Seq Scan` 单独作为失败。
- 低选择性查询、写密集表或已有前缀覆盖时，新增索引可能弊大于利。
- run/detail 查询已有主键或强 scope 时，可以合理豁免全局 time window。
- 单节点部署内存 stream 不自动构成缺陷；只有产品要求多节点/restart continuity 时才升级。
- 静态 growth `must_fix/later`、row estimate 或 index/table ratio 只是规划信号，不是当前 blocker。

## Severity

- `Blocking/High`：当前改动引入高增长无界查询、raw payload 无界列表读取、迁移/约束破坏、ephemeral 成为唯一真值、required/audit event 静默丢失，或候选绑定 fixture 出现确定性灾难计划。
- `Warning`：缺推荐索引但无 plan/规模证据；retention、分区、routing、多节点或容量压力尚属规划。
- `Unverified`：缺 candidate、统计新鲜度、fixture 数据分布、数据库环境或 plan evidence。

## Resource Boundary

- PR 默认只做静态 contract；catalog 与稳定 plan fixture 放 CI-beta/专项 QA。
- 只读 catalog 查询必须显式环境；未知环境禁止写 SQL、cleanup、自动建索引或修改统计。
- `EXPLAIN ANALYZE`、bloat/vacuum、load/soak 与生产统计不进入普通 PR。
- 复用既有报告；第一阶段不新增 query-plan 或 ephemeral inventory 自动化。

## Stop Conditions

- 需要改变 schema、索引、partition、retention、cache provider、stream 或多节点语义。
- 无法声明 query owner、数据规模、统计新鲜度或合法 fixture。
- 计划随 PostgreSQL 版本/统计波动，无法定义结构性断言；降级 observation。
- 需要读取生产用户 payload、日志正文、开启数据库扩展或运行破坏性语句。
