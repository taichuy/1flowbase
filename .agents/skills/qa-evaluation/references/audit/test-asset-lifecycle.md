# Test Asset Lifecycle Audit

## Goal

让测试持续证明仍有效且独立的 contract，而不是追求测试数量。识别短命、重复、弱断言和 harness 重叠候选，但不自动删除、合并或弱化验收。

## Invariants

```text
candidate → active → consolidation_due → deprecated → removed

TestValue heuristic =
  ContractRelevance × FailureAuthenticity × UniqueCoverage × DiagnosticPrecision
  / (ExecutionCost + MaintenanceCost)
```

- 当前 AC 必须由真实入口/行为证据结算；mock 次数、占位元素、固定字符串不能单独结算。
- 永久 contract test 不要求过期 metadata；temporary、skip/todo、baseline、compat 或计划合并资产才要求 owner 与期限。
- 例外 metadata 最小字段：`owner`、`purpose / contract-or-AC`、`merge_into`、`remove_by`。
- 删除前必须证明更强 replacement 已 green、原测试无独立 contract、诊断价值未丢失并由 owner 确认。
- 测试/harness 重复按生产入口、fixture、关键断言、资源 owner 和失败模式判断，不按标题或文本相似自动删除。

## Evidence

- 真实 red/controlled negative、被测生产入口、AC/contract、输入 fixture 与可观察断言。
- 测试 duration、flaky/retry、失败定位、churn、独立覆盖和共享 harness 资源。
- replacement 的参数化矩阵、集成/contract test、mutation 或等价失败证据。
- `repo-hygiene` 的 `.only`、skip/todo、弱断言、setup-only、identity-wrapper、重复标题和文件压力候选。
- harness inventory：进程、端口、session、credential、fixture、cleanup 与场景矩阵 owner。

## Legal Negatives

- 标题相同但 describe、route、状态、scope 或失败风险不同，不是重复测试。
- `toBeTruthy/toBeDefined` 正好表达 API contract 时，不是弱断言。
- 大型测试文件可能是有限完整 contract matrix；不能只因行数拆分。
- 两个 harness 共享 auth/helper，但协议、资源 ownership 或验收矩阵不同，可以并存。
- 小改动若改变可观察行为或修复历史回归，新增一个长期回归测试可能合理。

## Severity

- `Blocking/High`：`.only` 导致集合不完整；伪测试被用于结算当前 AC；harness 泄漏进程/session/credential；删除测试导致当前 contract 无证据。
- `Warning`：skip/todo、弱断言、低价值/重复候选、过期 metadata、短命测试未合并、harness 重叠、测试文件/时长压力。
- `Advisory`：参数化、矩阵合并、测试层级或运行分片优化机会。
- `Unverified`：无法确认生产入口、独立 contract、replacement 或 owner。

## Resource Boundary

- PR 仅阻断确定性的执行完整性和当前 AC 真实性；生命周期/重复候选保持 warning。
- Project Health 可聚合 repo hygiene、hotspot、duration/flaky 与过期 metadata；月度人工确认合并/删除。
- 第一阶段不新增 test-asset 自动化、mutation testing 或强制全测试 manifest。
- QA 不直接删除测试、harness、组件或抽象；任何删除另开实施 Issue。

## Stop Conditions

- 只能根据标题、行数、coverage 百分比或相似代码推断可删除。
- replacement 未 green、独立 contract 差异不清或 owner 未确认。
- 删除/合并会改变验收矩阵、公共 contract 或运行资源 ownership。
- 自动化收益不足以抵消误删和维护成本。
