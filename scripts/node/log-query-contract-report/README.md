# Log Query Contract Report

扫描日志、trace、ledger、artifact 查询入口的 `scope / time / cursor / limit` contract。

```bash
node scripts/node/tooling.js log-query-contract-report
```

报告输出：

- `tmp/test-governance/log-query-contract-report.json`
- `tmp/test-governance/log-query-contract-report.md`

规则：

- 缺少必需 contract 证据时为 `failed`，CLI 返回非 0。
- 非适用维度允许有理由的豁免；`removeBy` 必须为尚未到期的日期或明确的 `not_required`。豁免不得跳过配置中列出的实际证据。函数只读取自身 token tree，其他 helper / SQL 必须在配置中显式声明，邻居函数不能代为满足断言。
- run-scope/detail 查询可以豁免 time/cursor/limit，但必须说明它不是跨 run 的高增长列表。

## API Contract

- `scope`：高增长列表必须由后端强制带上 `workspace_id`、`application_id`、`flow_run_id`、`conversation_id`、`parent_trace_node_id` 或 artifact primary key 中的明确范围；前端筛选不能替代后端 scope。
- `time`：跨 run 的 application 级日志/监控查询默认使用 7 天窗口；`time_range_days <= 0` 按默认窗口处理，不能解释为全量。
- `cursor`：run 内事件流使用 exclusive `from_sequence`/`after_stream_sequence`，响应中的 `next_sequence` 可作为下一页 cursor；trace children 使用 opaque cursor，方向是 `order_key asc, trace_node_id asc`。
- `limit`：run logs `page_size` 最大 100，trace children 最大 100，JSON debug stream `limit` 默认 500 最大 1000，artifact batch resolve 最大 50 个 refs。
- 当前 `page/page_size` offset 是既有有效分页 contract：必须同时验证 application scope、默认 time window、page_size 和 SQL limit/offset、唯一 id tie-breaker 与其排序 helper；不靠过期的 cursor 迁移豁免继续通过。

## Related Reports

- `schema-hygiene` 负责物理表 schema 红线。
- `growth-table-report` 是显式容量 / 分区规划报告，其路由键准备度不冒充当前唯一性要求。
- `raw-jsonb-report` 负责 summary / preview / raw 读取边界。
- 本报告负责 API / repository 查询是否避免无界读取。
