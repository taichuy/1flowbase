# Growth Table Planning Report

`node scripts/node/tooling.js growth-table-report` 是显式运行的容量 / 分区规划报告，保留规划规则与测试，不纳入默认 `verify-repo tooling` 验收序列。

当前 PostgreSQL 存储不以物理分区键或分布式路由键作为所有唯一约束的前提。报告中的 `must_fix` / `later` 描述未来拓扑准备度，不代表当前 schema 不合法，不得据此改写主键、幂等唯一性或新增猜测性的容量限制。变更物理分区和唯一键语义需独立架构决策。

当前完整性继续由 `schema-hygiene` 验证，实际 API scope / 分页契约由 `log-query-contract-report` 验证，payload 读取边界由 `raw-jsonb-report` 验证，真实统计由 `capacity-report` 提供。索引建议须回到实际 filter / order / join、现有前缀和执行计划判断；推荐列形状本身不证明性能缺陷。

依据：[数据库查询与临时状态验收](../../../.agents/skills/qa-evaluation/references/audit/database-query-ephemeral.md)。历史设计始于 `7c1fc91b4`，配置已要求分区规划与主键语义变更分开决策；本次保留 CLI 作为规划入口，纠正默认验收将规划准备度表述为当前质量 warning 的混用。
