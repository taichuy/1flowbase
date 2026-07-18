# Budget Calibration

用于 Delivery 结束观测和跨 Delivery 聚合。目标是校准预测、定位耗时与返工来源，不把预算指标变成完成标准。

## Observation Contract

在 Delivery `integrated / reframed / blocked / cancelled` 时写入审计评论；字段不可见时写 `null`，不能用零或估算值代替：

```yaml
budget_observation:
  schema_version: 1
  class: {archetype: existing-codebase, grade: g4, domain: backend-contract}
  delivery: "#<id>"
  attempt_id: "#<id>/a1"
  supersedes_attempt: null
  evidence_tier: targeted
  execution_profile: {model: unknown, reasoning: unknown}
  timestamps: {started_at: null, candidate_at: null, ended_at: null, external_deadline_at: null}
  forecast_minutes: {initial_p50: null, initial_p80: null, latest_p50: null, latest_p80: null, confidence: low}
  phase_minutes: {probe: null, implementation: null, validation: null, review: null, integration: null}
  actual_minutes: {critical_path: null, agent_elapsed: null, external_wait: null}
  counts: {control_intervals: null, interval_overruns: null, agent_contexts: null, heavy_validations: null, rework_cycles: null, reforecasts: 0}
  usage: {input_tokens: null, output_tokens: null, billed_units: null}
  outcome: integrated | reframed | blocked | cancelled
  settled_root_ac: []
  variance_cause: scope | unknown | build | test | review | integration | external | none
```

- 保持 `initial_*` 不可变；只在新证据改变路径时更新 `latest_*` 并增加 `reforecasts`。
- `phase_minutes` 是非重叠受控 critical path 分段；`external_wait` 单列。只有工具或时间戳可观察的 usage 才记录数值。
- `integrated` 只有在结果进入 Root 集成基线并减少 Root AC 残差时成立；其他 outcome 不得因耗时短而成为成功样本。
- 每个控制尝试使用唯一 `attempt_id` 并只聚合一条最终记录；恢复 blocked / cancelled 工作时创建新 attempt 并指向 `supersedes_attempt`。相同 attempt 的重复评论按最新 `schema_version + ended_at` 去重，不把一次尝试重复计数。

## Eligible Cohorts

先按相同 `archetype + grade + domain + evidence_tier` 分组并按 attempt 去重，再为每项指标单独选择 cohort：

- `Coutcome`：`started_at` 非 `null` 且 outcome 合法的所有 attempt；用于 integrated / reframe / block / cancel rate。
- `C50`：`outcome=integrated`，且 `initial_p50 > 0`、`actual_critical_path` 非 `null` 且 `≥ 0`。
- `C80`：`initial_p80 > 0`，并满足 `outcome=reframed`，或满足 `outcome=integrated ∧ actual_critical_path` 非 `null` 且 `≥ 0`。只有 integrated 且 actual 不超过 initial P80 是 hit；reframed 固定是 miss。
- `Camp`：`critical_path > 0` 且 `agent_elapsed` 非 `null`、`≥ 0` 的 observation，不限制 outcome。
- `Cwait`：`critical_path` 与 `external_wait` 非 `null`、`≥ 0`，且两者之和大于零的 observation。
- `Ccount(x)`：对应 count 非 `null` 且 `≥ 0` 的 observation；interval overrun 还要求 `control_intervals > 0`。
- `Crework`：`rework_cycles` 非 `null` 的 observation，不限制 outcome。

`blocked` 与 `cancelled` 是 censored outcome，不进入 P50 / P80 校准；仍进入 Coutcome 的 block / cancel rate。任何 `null` 只排除依赖该字段的指标，不删除 observation，也不转换为 `0`。公式中的 `N` 始终是对应 cohort 的大小。

## Metrics And Decisions

```text
k50 = median(actual_critical_path / initial_p50 for C50)
coverage80 = count(integrated ∧ actual_critical_path ≤ initial_p80 in C80) / |C80|
agent_amplification = sum(agent_elapsed for Camp) / sum(critical_path for Camp)
external_wait_share = sum(external_wait for Cwait) / sum(critical_path + external_wait for Cwait)
outcome_rate(y) = count(outcome=y in Coutcome) / |Coutcome|
mean_count(x) = sum(count[x] for Ccount(x)) / |Ccount(x)|
interval_overrun_rate = sum(interval_overruns) / sum(control_intervals) for paired non-null counts
rework_rate = count(rework_cycles > 0 in Crework) / |Crework|
mean_rework_cycles = sum(rework_cycles for Crework) / |Crework|
phase_share(x) = sum(phase_minutes[x]) / sum(critical_path) for non-null paired values
```

- 每项指标独立按 `ended_at` 取最近 10 条 eligible observation；少于 5 条时只逐条报告原始值，forecast 保持 provisional / low confidence，不应用校准系数。
- eligible 样本达到 5 条后使用滚动窗口的 `next_P50 = raw_P50 × k50`；调整 P80 buffer，使 `coverage80` 接近 0.8。旧 observation 继续保留用于长期趋势，不进入当前窗口。
- P80 miss 若主要来自 `scope / unknown / review`，优先重构切片或 readiness；主要来自稳定执行时长偏差时才扩大时间 buffer。
- `latest_*` 只用于当前控制和 ETA，不参与历史预测准确度，避免用事后重估抹掉初始偏差。

## Deterministic Fixture

以下 observation 属于同一 class / evidence tier；其余字段不影响本 fixture：

| ID | outcome | variance | initial P50 | initial P80 | latest P80 | actual critical path |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| A | integrated | none | 60 | 100 | 100 | 80 |
| B | integrated | none | 120 | 180 | 180 | 150 |
| C | integrated | build | 120 | 180 | 300 | 240 |
| D | reframed | scope | 90 | 120 | 60 | 30 |
| E | blocked | external | 90 | 120 | 120 | null |

确定结果：

- `C50={A,B,C}`，`k50=median(80/60, 150/120, 240/120)=4/3≈1.33`。
- `C80={A,B,C,D}`；A、B hit，C 超过 initial P80，D 虽只耗时 30 分钟仍是 miss，因此 `coverage80=2/4=0.50`。
- E 是 censored outcome，不进入 C50 / C80；C 的 latest P80=300 不得把初始预测 miss 改写为 hit。

聚合实现或人工计算与上述结果不一致时停止校准，先修 cohort、forecast version 或 `null` 处理。
