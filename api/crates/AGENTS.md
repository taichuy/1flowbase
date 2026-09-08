# Scope

- 作用域：`api/crates/`；更深层 `AGENTS.md` 覆盖本文件的局部规则。
- 本文件集中维护 crate 依赖方向与一级目录 owner；宿主接入与跨目录生命周期边界继承 [api/AGENTS.md](../AGENTS.md)，业务细节留在对应 crate。
- 依赖记法：`A → B` 表示 `A` 可以直接依赖 `B`。

## Dependency Rules

- 依赖只能指向更稳定的 contract / domain，不得形成循环。
- 高扇出 crate 必须小且稳定；`domain`、`*-contracts` 不得吸收 service、SQL、宿主装配或通用杂项。
- 具体实现依赖抽象；contract 不得反向依赖 adapter、application、host 或 executable。
- 生产 crate 不得依赖 `*-tests`、`*-test-support`；跨层组合只放测试宿主。
- 不新增无明确 owner 的 `common`、`utils`、`helpers`、`manager` crate。
- 新 crate 必须同时具备独立职责、稳定公共边界和可验证的编译隔离收益；否则使用现有 crate 内部模块。
- 新增或调整内部 Cargo 边时，同步更新本文件和 dependency boundary test。

## Directory Owners And Allowed Internal Dependencies

| 目录 / crate | 唯一职责 | 允许的内部直接依赖 |
| --- | --- | --- |
| `domain` | 领域对象、不变量、状态与作用域语义 | 无 |
| `interface-runtime` | 协议无关的 Definition / Binding / compiled Plan、Registry snapshot、认证关联及 Invocation Kernel / Receipt / 有界收尾 | `domain` |
| `extension-contracts` | Host / Runtime 稳定 wire type、Slot 与协议错误 | 无 |
| `access-control` | 权限目录、角色与授权规则 | `domain` |
| `control-plane-contracts` | adapter-facing repository trait、持久化投影与 contract error | `domain`、`extension-contracts` |
| `observability` | 日志、trace、metrics 基础能力 | `domain` |
| `extension-package-runtime` | Runtime Host 所需的 package descriptor、解析、artifact load/reconcile | `extension-contracts` |
| `runtime-profile` | 运行目标、locale、fingerprint 与环境快照 | `extension-contracts` |
| `runtime-core` | runtime registry、runtime CRUD 核心、slot engine 与六个必实现 Runtime Backend Port | `domain`、`extension-contracts`、`storage-durable` |
| `orchestration-runtime` | 编排编译、绑定、执行与 `provider-routing` | `domain`、`extension-contracts`、`runtime-core` |
| `plugin-framework` | manifest、contribution、registry、安装与扩展图 | `access-control`、`extension-contracts`、`extension-package-runtime` |
| `runtime-extension-host` | RuntimeExtension Registry、Worker、stdio、profile 与生命周期的唯一运行真值 | `extension-contracts`、`extension-package-runtime`、`runtime-core`、`runtime-profile` |
| `runtime-extension-sdk` | RuntimeExtension 作者侧 typed Host Service client、Simulator 与 wire fixture | `extension-contracts` |
| `control-plane` | Use Case、状态写入口、事务、审计与应用策略 | `access-control`、`control-plane-contracts`、`domain`、`extension-contracts`、`observability`、`orchestration-runtime`、`plugin-framework`、`runtime-core`、`runtime-profile`、`storage-object` |
| `storage/durable/core` (`storage-durable`) | 与数据库实现无关的 Durable contract 与共享类型 | `domain`、`extension-contracts` |
| `storage/durable/postgres` (`storage-durable-postgres`) | PostgreSQL repository、SQL、事务、migration 与 mapper | `control-plane-contracts`、`domain`、`extension-contracts`、`storage-durable` |
| `storage/ephemeral` | session、cache、lease、lock、queue 等短期状态实现 | `control-plane-contracts`、`domain` |
| `storage/object` | 业务文件对象存储 driver | 无 |
| `control-plane-test-support` | Control Plane 测试 fixture 与 builder | 仅测试使用 |
| `postgres-test-support` | PostgreSQL 隔离 schema、migration 与数据库 fixture | 仅测试使用 |
| `control-plane-postgres-tests` | 真实 Control Plane + PostgreSQL 跨层验收 | 仅测试使用，不承载生产实现 |

## Placement Rules

- 业务决策、权限结果、状态流转、事务意图放 `control-plane`；纯领域不变量放 `domain`。
- adapter-facing repository trait 以 `control-plane-contracts/src/ports` 为唯一 owner；`control-plane/src/ports` 只保留业务端口或兼容重导出。SQL、Row mapper 和数据库事务实现放 `storage/durable/postgres`；actor/scope 查询过滤属于 repository，状态流转和审计写入属于 `control-plane`。
- 关键业务写动作从 `control-plane` 的命名 service command 或 Resource Action Kernel action 进入；跨层真实 service/PostgreSQL 验收放 `control-plane-postgres-tests`，生产存储 adapter 不反向依赖业务实现。
- 跨 Host / Runtime 的稳定协议放 `extension-contracts`；安装、registry 和扩展图放 `plugin-framework`。
- lifecycle subscriber 的 typed handler binding/registry 由 `plugin-framework` 编译，`api-server` Composition Root 从 active HostExtension 的 native entrypoint factory 注入 binding；delivery adapter 不得按 handler id 硬编码插件实现或以 EventBus enqueue 代替 handler 完成。受管 RuntimeExtension/CapabilityPlugin 仅通过稳定 typed managed transport 绑定已开放的 Create committed / processed 事件与 Invocation / WorkspaceAssignment 作用域；User 作用域不开放，不能绕过贡献级授权。
- RuntimeExtension 加载与进程生命周期放 `runtime-extension-host`；执行编排放 `orchestration-runtime`。`extension-package-runtime` 仅负责描述符解析和 artifact load/reconcile，不接管安装、签名或扩展图；Runtime Backend Port 只接收 artifact reference，不泄漏本机 package path。
- `interface-runtime` 拥有协议无关的认证 attempt 关联、冻结计划执行、调用取消/超时和终态收尾；不拥有凭证解析、业务授权策略、业务事务、Outbox 投递或 Runtime Host。局部不变量与反例见 [interface-runtime/AGENTS.md](interface-runtime/AGENTS.md)。
- Effective Extension Graph 是声明输入；Composition Root 投影并绑定 typed ports，compiled Dynamic Interface Registry 是 active definition 真值。Kernel 执行冻结的授权/准入 veto 与 Hook，不编译宿主 Graph 或加载插件。
- `RuntimeBackend` 必须组合 Execution、Observation、Provider、DataSource、Capability、Network Egress 六个窄 Port；必需方法不提供默认失败实现。
- `orchestration-runtime` 只持有 `RuntimeExecutionPort`；完整 `RuntimeBackend` 仅停留在 `api-server` composition root 与业务能力装配层，窄 Port 必须由该层从 exactly-one Slot Backend 内部投影，装配构造器不得接收第二个独立 Port 来源。
- Provider Distribution Rule 属于 `RuntimeExecutionPort` 的 typed operation，不新增第七 Port；Host 只执行插件 decision，`orchestration-runtime` 独占 eligible target 校验、retry 与 Provider invocation。
- Durable、Ephemeral、Object 是三类 Storage；PostgreSQL 只是 Durable 的官方 adapter。
- Plugin Managed Data Model 只接受 manifest 的 additive desired state；`control-plane-contracts` 持有窄 Port，`plugin-framework` 编译 plan，PostgreSQL adapter 独占 catalog/DDL/ownership。目标业务表不得维护 `extension_field_slot` 或第二套 allowlist；RuntimeExtension 不得获得 SQL 或数据库连接。
- `PluginDataPort` 是 Host Service，不进入 RuntimeBackend 六 Port。worker frame 不携带 plugin/workspace/actor 身份；Host 从 manifest 与内部 execution principal 注入，PostgreSQL adapter 仅按 ownership ledger 解析物理对象。

## Evidence And Stop

- 依赖调整至少检查 `cargo metadata` 和 `control-plane-postgres-tests --test dependency_boundaries`。
- 若新依赖要求 contract 调用具体实现、复制 canonical DTO、改变 API/schema/权限/runtime 语义，停止并返回架构 Root 重新定界。
