# Gate execution and evidence reuse

[中文](README.md#门禁执行与结果复用)

Full project acceptance uses `clippy --all-targets` to type-check the same package
groups. Its CI matrix and required receipts omit the three redundant `cargo check`
shards. The PR baseline has no Clippy step and keeps those check shards. Explicit
check commands remain available. Clippy does not replace feature-specific gates
or behavior tests.

`scripts/node/verify/backend.js` owns Cargo command construction and validation of
protected test executions. `test-backend` reuses its static check, image/multimodal
scenarios, and workspace test plan. Each protected command still requires actual
passing tests: adjacent success, ignored tests, compilation, or absent output do
not establish execution evidence.

`tooling data-contracts` shares one migration inventory and source analysis results
across the existing schema, raw JSONB, query-contract, and capacity JSON/Markdown
reports. Schema errors and query-contract failures still block. Raw JSONB findings
and growth planning remain advisory, and missing capacity measurements remain
unverified. Missing or malformed required policies, including query contracts,
still fail. Standalone report commands remain available for targeted diagnosis.

Reuse between targeted and full suites requires equivalent tests and execution
conditions. Differences in nextest/libtest, stack size, features, and coverage
instrumentation need separate assessment. Consistency, foundation, coverage, and
protected image-llm scenarios remain in the acceptance plan.
