# Backend Regression Steps

只要评估范围涉及后端 API、状态入口、插件边界、runtime、`Resource Action Kernel`、HostExtension registry 或 `route / service / repository / domain / mapper` 分层，就必须按以下顺序做后端回归；Dev Acceptance Gate 按最小证据链裁剪，PR / Project Health Gate 再扩大覆盖。不要跳过前置验证直接下 QA 结论，也不要把运行态取证当成默认步骤。

## Contents

- [Fixed Order](#fixed-order)
- [Step 1: Read Specs First](#step-1-read-specs-first)
- [Step 2: Run Backend Verification](#step-2-run-backend-verification)
- [Step 3: Sample Key Routes](#step-3-sample-key-routes)
- [Step 4: Sample Service Write Entrypoints](#step-4-sample-service-write-entrypoints)
- [Step 5: Sample Repository And Mapper Layering](#step-5-sample-repository-and-mapper-layering)
- [Step 6: Blast Radius Before Conclusion](#step-6-blast-radius-before-conclusion)

## Fixed Order

1. 读相关 spec
2. 跑后端验证命令
3. 抽样检查关键路由和接口质量门禁 evidence
4. 抽样检查 service 写入口
5. 抽样检查 repository / mapper 分层
6. 最后再给 QA 结论

## Step 1: Read Specs First

至少补齐：

- `api/AGENTS.md`
- 当前任务说明、改动范围、验收标准
- 与本次范围直接相关的后端项目记忆

如果存在直接相关的 spec / plan，再补齐对应文件；不要把过期 spec 当成默认真相来源。

如果涉及插件、runtime 或动态建模，必须额外确认：

- `public / control / runtime` 三平面归属
- `HostExtension / RuntimeExtension / CapabilityPlugin` 边界
- `Resource Action Kernel` 是否仍由宿主托管
- HostExtension 是否只通过 manifest contribution 注册 resource、action、hook、route、worker、migration 和 infrastructure provider
- pre-state infrastructure provider 是否在 `ApiState`、session store、control-plane service、runtime engine 和 HTTP router 构造前完成
- Provider / 上游 runtime 错误是否按 passthrough contract 保留原始排障信息；不要把 stdout / stderr / upstream error 进入 `RuntimeContract` / API response 判为泄漏，真正的问题是宿主改写、截断、翻译、吞掉或泛化上游信息
- RuntimeExtension / CapabilityPlugin 是否没有直接持有 Redis、NATS、RabbitMQ 等基础设施连接
- native HostExtension 是否保持 in-process、restart-scoped，不设计 Rust native 热卸载
- `dynamic modeling` 是否仍是元数据系统，而不是 runtime 数据本身
- `scope_kind` 是否只保留 `workspace/system`
- `system` 是否固定使用 `SYSTEM_SCOPE_ID`
- runtime 物理 scope 列是否统一为 `scope_id`
- 活跃后端代码是否不再使用 `team/app` alias、`team_id/app_id` 表示 scope
- Application 领域新增命名是否使用 `application_id`，而不是 `app_id` 缩写
- 如果涉及文件管理，`file_storages` 是否仍归 `root/system` 管理，文件记录是否仍保存实际 `storage_id` 快照

## Step 2: Run Backend Verification

优先运行与当前风险直接对应的后端验证脚本；Dev Acceptance Gate 默认复用 TDD 红绿结果，只补一个主验证命令和必要 smoke。PR / Project Health Gate 才默认考虑仓库级后端验证。
同一工作区内的 `cargo` 验证命令默认串行执行，不要并发启动多条 `cargo test / check / clippy`，否则容易卡在 `package cache` 或 `artifact directory` 锁上，拿不到稳定 QA 证据。

PR / Project Health Gate 优先：

```bash
node scripts/node/verify-backend.js
```

如果只需要先确认 Rust 静态门禁：

```bash
node scripts/node/tooling.js check-rust-backend
```

仓库级最小验证：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace
```

如果只改单一 crate，Dev Acceptance Gate 至少补：

```bash
cargo test -p <crate-name>
```

如果需要补多个 crate 的 targeted tests，也按串行顺序逐条执行，并在记录里保留每条命令对应的结果，不要把锁等待当成测试通过。

无法执行时，必须在报告里明确说明为什么要跑、为什么没跑成、因此哪些结论只能停留在 `未验证`。

## Step 3: Sample Key Routes

至少抽查每个被改动或被影响平面中的关键路由。QA 只读取已确认的验收预期、OpenAPI、DTO、测试或现有 contract；缺少预期时标为 `未验证`，不要在 QA 阶段临时发明接口语义。

- 预期来源：method / path / plane、认证方式、CSRF 要求、请求 DTO、预期 status、response DTO / error shape、状态副作用、审计或事件
- 认证态：in-process route integration 优先复用项目测试 support 的登录 / session / CSRF helper；运行态请求先调用 `/api/public/auth/providers/password-local/sign-in` 获取 session cookie 和 `data.csrf_token`，mutating console request 带 `cookie` 与 `x-csrf-token`
- 运行态取证：仅当需要真实服务、认证链、环境配置、线上 / 本地差异或手工复现证据时，使用 `node scripts/node/tooling.js api-debug [METHOD] <api-path-or-url> --expect-status <code>`；同一 route contract 已被 integration test 覆盖时，不默认重复运行。该工具从 api-server `.env` 读取 root 账号密码，自动登录并为任意 API 请求带认证态
- evidence 记录：保留请求摘要、status、脱敏 headers、response body 关键字段、执行命令或测试名；原始 artifact 放 `tmp/test-governance/`，不得记录 cookie、token、secret 或密码
- 路径是否仍放在正确平面
- 是否保持 `ApiSuccess` / `204 No Content` / 统一错误结构
- 认证、ACL、审计和 OpenAPI 暴露是否仍由宿主管理
- 公共 API 契约变化后，调用方和相关回归是否同步成立
- Project Health Gate 场景使用 mock / fixture / 受控数据跑接口质量门禁，检查状态是否正常、返回结构是否稳定、值是否正确、过期 / 禁用 / 缺失状态是否符合已确认预期
- 如果没有运行态服务或脚本，route integration 测试可作为 API evidence；只测 service / repository 不能单独证明 route contract 成立

## Step 4: Sample Service Write Entrypoints

至少抽查关键状态写入口，确认：

- 状态修改是否仍通过命名明确的 service action / command
- route 没有绕过 service 直接改状态
- HostExtension route / worker 没有绕过 `Resource Action Kernel` 直接改 Core 真值
- repository 没有偷偷承担事务意图、权限判定或状态流转
- 关键副作用、审计、幂等仍由 service 编排
- `workspace`、`system` 与 session scope 语义没有在写入口被重新混用

## Step 5: Sample Repository And Mapper Layering

至少抽查一个关键 `repository + mapper` 配对，确认：

- `repository` 只做持久化与查询投影，不偷带业务逻辑
- `mapper` 只做转换，不藏权限、状态或额外查询语义
- `storage-durable/postgres` 内的 `storage-postgres` repository / mapper 拆分仍然成立
- `storage-durable` 没有吸收额外 durable backend 细节，`storage-object` 没有混入插件产物存储或业务 service 规则
- 复杂 SQL、JSON 字段、枚举转换等易错点有对应 targeted tests
- runtime metadata、物理表列名与 `scope_id` 语义保持一致

## Step 6: Blast Radius Before Conclusion

出 QA 结论前，至少补一轮后端 blast radius 审查：

- 公共 API、session 或 auth 契约变化后，调用方是否同步成立
- `storage-durable/postgres`、`storage-durable`、`storage-object` 或持久化层调整后，service、route、tests 是否仍成立
- runtime 或插件相关改动后，白名单槽位与消费方式是否仍成立
- HostExtension 相关改动后，manifest contribution、load plan、route / worker / migration namespace 和 infrastructure provider 是否仍成立
- `storage-durable/postgres/migrations` 是否仍是顺序追加历史迁移链；修改历史 migration 时是否使用独立 schema 避免 checksum 污染
- `workspace/system`、`SYSTEM_SCOPE_ID` 与 runtime `scope_id` 约束是否贯穿 route / service / repository / tests
- `_tests`、文件大小、目录收纳和最小验证命令是否仍遵守质量门禁

如果以上任一步没有证据，结论必须降级为：`未验证，不下确定结论`。
