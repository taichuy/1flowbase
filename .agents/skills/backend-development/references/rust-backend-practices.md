# Rust Backend Practices

## Principle

类型表达不变量，错误显式传播，状态机封装转换，副作用集中在边界，异步代码避免隐式阻塞，生产请求路径少用 `panic` / `unwrap`。

## Core Rules

- 请求处理路径不用 `unwrap()` / `panic!()`；测试、启动期配置和硬编码不变量可用 `expect()`，但必须写清失败原因。
- 错误类型区分业务错误和系统错误；底层 `sqlx::Error`、外部依赖错误和 `anyhow::Error` 不直接穿透到 API 契约。
- 重要业务概念用 newtype，不在领域核心裸用 `String` / `Uuid` / `i64`；越靠近核心，类型越具体。
- 状态变化只能通过领域方法、service command 或 action 入口；字段默认私有，不允许外部直接改状态字段。
- `Option` 表示可缺失，`Result` 表示可失败；不要用 `Option` 吞掉数据库错误或外部依赖失败。
- handler 保持薄，只做协议解析和响应映射；service 编排权限、事务、幂等和副作用；domain 保证业务规则；repository 只做持久化和查询投影。
- DTO、Domain、DB Row 不默认共用；敏感字段、内部字段和 API 返回结构必须显式映射。
- `Deserialize` 只保证输入形状；业务校验进入 command/domain 类型，例如 `TryFrom<Request>` 构造命令。
- 一个用例内同时改变多个持久化状态时，必须明确事务边界；不要在数据库事务里调用慢外部服务。
- 会产生副作用的外部接口要考虑幂等 key、唯一约束或可解释的重复请求处理。
- 重要不变量双层保护：Rust 类型 / 业务逻辑 + 数据库唯一索引、外键、`CHECK` constraint。
- async 请求路径不做阻塞 IO；CPU 密集任务使用 `spawn_blocking`、独立 worker 或后台队列。
- 不要跨 `.await` 持有锁；锁内只做短同步操作，必要数据先复制出来再 await。
- 函数默认借用，只在确实需要持有时获取所有权；不要用无意义 `.clone()` 回避所有权设计。
- 配置启动时集中加载、集中校验、显式注入；运行路径不要散落读取环境变量。
- 外部请求、关键业务动作和错误路径使用结构化 tracing；密码、token、密钥和敏感 payload 不进日志。

## Security And Scope

- Rust 内存安全不等于应用安全；资源访问必须显式带上 `actor` / `current_user` / `tenant_id` / `scope_id`。
- 多租户、system scope、workspace scope 和 session scope 不能靠调用方约定隐式传递。
- 权限、所有权校验和审计放在 service/action 入口，不藏在 repository 或 mapper。

## AI Collaboration Guardrails

AI 生成 Rust 后端代码时，重点检查这些核心坏味道：

- 领域核心大量出现 `String`、`serde_json::Value`、`HashMap<String, String>`、裸 `Uuid`
- `pub` 字段暴露状态，外部可任意改写
- `anyhow::Result` 在领域层或 API 契约层泛滥
- `.clone()`、`.unwrap()`、`dbg!()`、`todo!()` 用来快速过编译
- handler 内直接写事务、权限、状态流转或外部副作用
- repository 偷偷承担事务意图、权限判定或状态转换

## Minimum Test Expectations

- domain unit test 锁定状态机和不变量。
- service test 覆盖用例流程、权限、事务和幂等。
- repository test 验证 SQL、migration、唯一约束、外键和 check constraint。
- API integration test 保护 HTTP 行为、错误码、序列化和安全字段。

## Completion Self-Check

Rust 后端开发完成前，必须逐项回答：

- 这个错误会不会被吞掉？
- 这个状态能不能被非法构造？
- 这个字段能不能被外部乱改？
- 这个 ID 会不会传错类型？
- 这个请求重试会不会产生重复副作用？
- 这个事务失败会不会留下半成功数据？
- 这个 async 里面有没有阻塞？
- 这个锁有没有跨 await？
- 这个 API 会不会泄漏内部字段？
- 这个数据库约束能不能兜住并发问题？

完成口径不是“代码能编译”，而是能说明：

- 非法状态不可表示。
- 非法转换无法绕过。
- 错误路径必须处理。
- 副作用边界清晰。
- 并发风险被显式管理。
- 数据一致性由类型、事务和数据库约束共同保证。

如果任何一项暂时做不到，必须把原因、风险和补救位置写清；不要把未完成项包装成已完成。

## Minimum Gate Expectations

后端 Rust 改动先按当前任务选择最小结果验证：

- Dev Acceptance 默认优先运行相关 crate / module 的 targeted test，以及能证明 API / 状态 / repository 行为的最小 integration test。
- 同一 worktree 同时只启动一条 Cargo 命令，避免 package cache / artifact lock 竞争；该命令内部默认读取机器逻辑 CPU 的一半并行编译，不固定为单任务或常量 job 数。仓库包装命令会自动读取；直接运行定向命令时使用 `CARGO_BUILD_JOBS="$(node scripts/node/testing/verify-runtime.js cargo-jobs)" cargo ...`。
- workspace 级 `fmt` / `clippy` / `test` / `deny` 属于 PR / beta / Project Health 级质量门禁，不是当前本地开发分支默认收尾动作。
- 如果本地确实需要提前运行 workspace 级门禁，必须在对齐 / 已批准计划 / handoff 阶段说明证据收益、成本和不可延后的原因；实现期临时发现时默认标为未验证并交给 beta / CI，除非缺少该证据会影响继续实现安全性或当前任务完成判断。

PR / beta / Project Health 级后端 Rust 门禁至少考虑：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

按仓库脚本存在情况优先使用项目包装命令。`clippy::unwrap_used`、`clippy::expect_used`、`clippy::dbg_macro`、`clippy::todo` 可按 crate 风险分级启用；启动期和测试代码允许有明确例外。
