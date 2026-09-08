# Root #1998 第一阶段接口生命周期验收矩阵

> 历史记录：仅对文中冻结版本和当时范围负责，不作为当前架构或最新验收状态。当前说明见[架构索引](../../README.md)，同阶段记录见[归档索引](../README.md)。

> 历史fixture装配记录，以下状态保留当时语境。当前架构说明从[本地系列](../../interface-lifecycle/README.md)阅读；最终验收以对应候选报告为准，本文不随文档迁移改写为通过。

状态：**fixture 已装配，等待冻结候选、集中 QA 与同 SHA CI；没有 AC 在本文标为通过。**

本次只验收接口生命周期管理。目标规范为 Wiki `Request-Architecture-and-Invocation-Lifecycle-CN@207ec12`；原接口行为基线为 `6824b2c1701eacc5cd06f155b5536b6442b54220`。P1–P5 输入 `2b21a094f15b79d1e36ded2a05efe113e5c82c1c`，P6 fixture 提交 `c4f9cd5963ac6a4205dab044947eef1cc713f776`；最终候选 SHA 由 Root 在 P7 提交后冻结，本文不预写未知 commit 或测试结果。

## 当前阶段与责任

- HTTP/MCP 真实路由验证核心业务输入、输出、授权和持久化等价；合法空扩展计划是正常输入。
- Kernel/Registry 受控 typed registration 验证已绑定计划执行与冻结身份。该证据不声称真实 HostExtension manifest/loader 已向数据模型创建提供扩展。
- 业务事务拥有 commit/rollback；Invocation Receipt 拥有调用终态；协议 writer 拥有响应写入；Outbox delivery owner 拥有 subscriber acknowledgement。
- 不新增插件 manifest、native factory、生产错误码、迁移或 Runtime 拓扑。原 P6a 已撤下。完整插件时空组合性、Outbox lease/retry/reclaim 与 PluginData 幂等专项留到第二阶段。

## 有限验收矩阵

表中函数均为实际 Rust/Node test inventory 名称。API、Kernel 与 PostgreSQL 三类证据互不替代。

| AC | 测试函数 | 可观察断言 | 执行状态 |
| --- | --- | --- | --- |
| AC-005 | `root_1998_ac_005_http_mcp_create_success_and_core_deny_preserve_baseline` | 等价隔离 schema/账号权限与输入；实际 POST 和 MCP tools/call；基线 DTO keys/固定字段、HTTP 201/403、MCP 错误类别；规范化完整返回与落盘字段；拒绝前后模型/字段/grant 行摘要不变；冻结配置与目标计划 pins | 待 CI |
| AC-005 | `root_1998_ac_005_required_input_preserves_http_and_mcp_ingress_contract` | 缺失 `scope_kind`：HTTP Json ingress 422；MCP required mapping 的 `-32602 / invalid_tool_arguments / mcp_arguments`；不产生业务写入 | 待 CI |
| AC-006 | `root_1998_ac_006_empty_plan_executes_core_and_records_terminal_receipt` | 无 executable hooks 时正常经过 core lifecycle，返回真实终态 Receipt | 待集中 QA |
| AC-006 | `root_1998_ac_006_rr15_rr16_unary_and_stream_execute_core_then_ordered_veto_then_hook_handler` | 既有 unary/stream core→ordered Authorization/Admission→Before→Handler 次序不被调用方绕过 | 待集中 QA |
| AC-006 | `root_1998_ac_006_rr15_core_deny_dominates_plugin_allow_and_extension_failures_fail_closed` | core deny 不能恢复；受控扩展拒绝/执行故障不能降级为允许 | 待集中 QA |
| AC-006 | `root_1998_ac_006_rr15_rr16_decision_bindings_fail_publish_when_missing_extra_or_contract_mismatched` | 缺失、额外及错误 typed contract 在 registry publication 前拒绝 | 待集中 QA |
| AC-006 | `root_1998_ac_006_in_flight_snapshot_keeps_plan_handler_and_completion_identity` | 旧调用已 poll 到 Before barrier，再发布新 snapshot；旧/新输出、Graph/Registry/Plan/Handler pins 和 Completion 属于各自快照 | 待集中 QA |
| AC-007 | `root_1998_ac_007_commit_and_completion_do_not_ack_or_reverse_failed_delivery` | 真实 ModelDefinitionService→PG 创建与 required fact 提交；Kernel Completion 已执行，但 subscriber 仍 pending/attempt=0；serde writer 返回 BrokenPipe 后数据、fact 和 Receipt 保持 | 待 CI |
| AC-007 | `root_1998_ac_007_commit_rejection_rolls_back_model_schema_and_required_fact` | 指定 fixture fact 的 deferred constraint 在真实 COMMIT 报错；service 错误进入 Failed Receipt；模型、事务 DDL、字段、fact、delivery/grant 无新增残留 | 待 CI |
| AC-007 | `root_1998_ac_007_cancelled_invocation_does_not_mean_business_rollback` | 真实 service 已提交后暂停 Handler 返回，取消调用；Receipt=Cancelled、Completion 执行，业务/fact 仍存在且尚未 ack | 待 CI |
| AC-007 接线 | `root_1998_ac_007_cross_layer_postgres_host_enters_all_four_ci_partitions` | 四个既有 PG hash 分片均包含原 adapter 与跨层测试宿主，确定性命令不丢失原 package | 待集中 QA |

A1–A4 继续使用 #1999/#2000 已提交的 endpoint、WebMCP、finalization、authentication fixture 和 Root Test Batch；本文件不重复声明其结果。既有 graph cycle/缺失/contract negatives 由 Root 的 plugin-framework extension_bus 命令执行，无新 loader fixture。

## 迁移前输入输出锚点

P6 的断言以基线源码与既有测试为依据，不能只比较两个新路径：

- `api-server/src/_tests/route_docs/docs_routes.rs::create_model`：POST 路径、请求字段、HTTP 201 与 `data.id`。
- `api-server/src/_tests/mcp_protocol_routes.rs::ac_001_runtime_mcp_write_uses_server_delegation_without_browser_csrf` 与 `ac_002_runtime_mcp_write_keeps_console_operation_authorization`：成功业务 DTO 与 `403 / target_authorization / console_operation_permission_denied`。
- `routes/plugins_and_models/model_definitions.rs::{CreateModelDefinitionBody,ModelDefinitionResponse}`、`response.rs::ApiSuccess`、`error_response.rs::ErrorBody`：required input、响应 key 集合与 envelope。
- `routes/settings/mcp_management/debug_execute.rs::build_interface_arguments`、`routes/mcp_protocol/virtual_ui.rs::interface_error`：缺失 required mapping 的 `mcp_arguments` 分类。HTTP 保持基线 required String DTO 的 Axum Json ingress 拒绝。
- 原 `main_source_defaults` migration 的 published 默认值与 DTO runtime availability 映射保持固定预期。

规范化只处理生成的模型/字段 ID、等价隔离 workspace/actor ID、持久化 audit 时间；不删除业务字段来掩盖差异。字段按 code 排序以消除独立生成 ID 的排序影响。HTTP/MCP 的协议 envelope、outer Invocation ID 与 ingress 错误形态本来不同，按各自旧契约断言。

API fixture 的 pins 来自实际 Router 发布后持续保持的 frozen registry；真实在途 Receipt/Handler/Completion pins 由独立 Kernel barrier fixture 验证，不声称 API 响应暴露了 Receipt。

## 事务 fixture 的受控边界

`control-plane-postgres-tests/tests/interface_lifecycle_acceptance/` 使用 `PostgresTestSchema`、正式 migrations、真实 `ModelDefinitionService` 和 `PgControlPlaneStore`。测试宿主的 `interface-runtime` dev-dependency 只负责将 service handler 放入真实 Kernel；生产 crate 依赖不变。

required fact publication catalog 是有限测试配置，只有 `model_definition.committed@v1` 和一个 subscriber。测试不手工合成成功业务行，也不启动 dispatcher；pending 状态是“Completion 不等于 ack”的直接断言。

回滚触发器只存在于隔离 schema，且只对 `root_1998_rollback` 对应 fact 生效。它在模型、字段、DDL、change log、outbox、delivery 已进入同一事务后，于 COMMIT 报错。这不修改 adapter 在较早语句失败时保留 broken metadata 的既有恢复语义，也不把该恢复行为错误地描述成整次 service 全部原子回滚。service 的后续审计与 grant 写入并未被本 fixture 宣称为单一事务。

BrokenPipe 使用真实 serde JSON 写入路径与受控断连 writer，证明提交后的协议交付失败不能改写业务提交或 Receipt；它不证明真实网络 exactly-once、客户端已收到、自动重试安全性或远端 subscriber ack。取消 fixture 同样只证明 Cancelled 不能当作 RolledBack。

## 执行与证据位置

开发 Packet 不运行行为测试。允许的 test-target compilation、rustfmt、diff 与 Node syntax 只属于机械装配证据，不结算 AC。

Root 在同一冻结候选执行：

```sh
cargo test --manifest-path api/Cargo.toml -p interface-runtime --locked --offline
cargo test --manifest-path api/Cargo.toml -p plugin-framework --lib extension_bus --locked --offline
node scripts/node/interface-lifecycle-boundary/cli.js
node --test scripts/node/interface-lifecycle-boundary/_tests/core.test.js scripts/node/verify/_tests/interface-runtime-boundary.test.js scripts/node/verify-backend/_tests/cli.test.js
```

API 与 PG 行由 `.github/workflows/quality-gate.yml` 的 `scope=ci`、同一候选 SHA 提供真实执行证据。四个 `storage-postgres-N-of-4` hash 分片现在同时选择 `storage-durable-postgres` 与 `control-plane-postgres-tests`；新增 `interface_lifecycle_acceptance` Cargo target 因此进入实际 nextest inventory。原跨层宿主既有 target 作为影响面回归随之执行，不计作第二阶段插件专项验收。

Root 必须核对候选 SHA、required rows、新测试实际执行与 aggregate artifacts，不能仅引用编译成功或 workflow 绿色。日志、warning 和 coverage 统一在 `tmp/test-governance/1998/` 及候选 CI artifacts。最终 PASS/FAIL、风险和 candidate-bound run 链接由 Root 在证据产生后记录。
