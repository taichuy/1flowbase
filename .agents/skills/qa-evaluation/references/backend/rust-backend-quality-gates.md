# Rust Backend Quality Gates

## Contents

- [When To Use](#when-to-use)
- [Review Checklist](#review-checklist)
- [Evidence Chain](#evidence-chain)
- [Test Coverage Expectations](#test-coverage-expectations)
- [Completion Self-Check Evidence](#completion-self-check-evidence)
- [Failure Signals](#failure-signals)

## When To Use

评估范围命中 Rust 后端 API、service、domain、repository、migration、异步任务、状态入口或数据库一致性时，使用本文件补充 `backend-regression-steps.md`。

## Review Checklist

- 请求路径是否没有 `unwrap()` / `panic!()`；允许的 `expect()` 是否只在测试、启动期或不可恢复不变量，并带清晰原因。
- API 错误是否映射为稳定应用错误；底层数据库、外部依赖和内部错误没有直接泄漏到响应契约。
- 领域核心是否用 newtype / enum 表达业务概念和有限状态；重要字段是否默认私有。
- 状态转换是否只能通过领域方法、service command 或 `Resource Action Kernel` action；没有 handler / repository 绕过主入口改状态。
- `Option` 是否只表达可缺失，`Result` 是否表达可失败；数据库错误、权限错误和外部依赖失败没有被 `Option` 吞掉。
- handler / service / domain / repository / DTO / DB Row 是否职责分离；敏感字段没有因复用结构泄漏到 API。
- 输入是否从 DTO 转换到 command/domain 后再执行业务校验；没有只依赖 `Deserialize` 当业务验证。
- 多表或多状态写入是否有明确事务边界；事务内没有慢外部调用。
- 支付、创建、兑换、webhook、发信等副作用接口是否有幂等策略或唯一约束。
- 关键不变量是否同时有 Rust 侧校验和数据库唯一索引、外键、`CHECK` constraint 兜底。
- async 路径是否没有阻塞 IO；CPU 密集任务是否移出请求运行时或使用 `spawn_blocking`。
- 锁是否没有跨 `.await` 持有；共享状态更新是否短、可解释。
- 资源访问是否显式带 `actor` / `current_user` / `tenant_id` / `scope_id`，并在 service/action 入口做权限与审计。
- 结构化 tracing 是否覆盖关键请求、业务动作和错误路径；敏感信息没有进入日志。

## Evidence Chain

先选最小证据链，不为显得全面叠加无新增覆盖面的命令：

```bash
node scripts/node/test-backend.js
```

`test-backend` 和 `verify-backend` 会先运行：

```bash
node scripts/node/tooling.js check-rust-backend
```

该静态门禁会硬拦新增的生产路径 `unwrap` / `panic` / `dbg` / `todo` / `unimplemented`、敏感字段序列化和敏感日志；阻塞 IO 等高误伤项先作为 warning 写入 `tmp/test-governance/rust-backend-static-gate.json`。历史债由 `scripts/node/check-rust-backend/baseline.json` 兜住；新增命中不应追加 baseline，除非明确登记为阶段性技术债。

如果需要直接落到 Cargo，串行运行：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
```

如果范围只命中单个 crate，至少补：

```bash
cargo test -p <crate-name>
cargo clippy -p <crate-name> --all-targets -- -D warnings
```

如果修改依赖许可证、安全策略或新增供应链依赖，再补：

```bash
cargo deny check
```

命令失败或无法运行时，报告必须写明命令、失败原因、缺失证据和因此降级的结论。

## Test Coverage Expectations

- domain unit test 覆盖非法状态不可构造、非法转换不可绕过。
- service test 覆盖权限、事务、幂等、失败回滚和副作用编排。
- repository test 覆盖 SQL、migration、唯一约束、外键、`CHECK` constraint 和 mapper 转换。
- API integration test 覆盖 HTTP 状态码、统一错误结构、序列化、鉴权和敏感字段不泄漏。

## Completion Self-Check Evidence

Rust 后端验收时，必须能用代码、测试、数据库约束或运行结果回答：

- 错误是否会被吞掉。
- 状态是否能被非法构造。
- 字段是否能被外部乱改。
- ID 是否会传错类型。
- 请求重试是否会产生重复副作用。
- 事务失败是否会留下半成功数据。
- async 路径是否存在阻塞。
- 锁是否跨 `.await`。
- API 是否会泄漏内部字段。
- 数据库约束是否能兜住并发问题。

通过口径：

- 非法状态不可表示。
- 非法转换无法绕过。
- 错误路径必须处理。
- 副作用边界清晰。
- 并发风险被显式管理。
- 数据一致性由类型、事务和数据库约束共同保证。

没有直接证据时，对应项写 `未验证`；发现做不到时，按风险级别进入 Findings，不得用“后续优化”稀释状态一致性或数据一致性问题。

## Failure Signals

- `String` / `Uuid` / `serde_json::Value` 从 HTTP 一路穿到领域核心或数据库写入。
- `anyhow::Result` 在领域层、service 契约或 API 契约里泛滥。
- `.clone()` 只是为了修编译错误，函数本可借用。
- repository 里出现权限、审计、状态跳转或事务编排。
- handler 内直接做复杂业务、跨多表写入或外部副作用。
- 数据库约束缺失，只靠 service 先查再写防并发冲突。
