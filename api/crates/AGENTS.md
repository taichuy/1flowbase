# Scope

- 作用域：`api/crates/`；更深层 `AGENTS.md` 覆盖本文件的局部规则。
- 本文件只定义 crate 依赖方向与一级目录 owner；业务细节留在对应 crate。
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
- repository trait 放 `control-plane-contracts`；SQL、Row 映射和数据库事务实现放 `storage/durable/postgres`。
- 跨 Host / Runtime 的稳定协议放 `extension-contracts`；安装、registry 和扩展图放 `plugin-framework`。
- RuntimeExtension 加载与进程生命周期放 `runtime-extension-host`；执行编排放 `orchestration-runtime`。
- `RuntimeBackend` 必须组合 Execution、Observation、Provider、DataSource、Capability、Network Egress 六个窄 Port；必需方法不提供默认失败实现。
- `orchestration-runtime` 只持有 `RuntimeExecutionPort`；完整 `RuntimeBackend` 仅停留在 `api-server` composition root 与业务能力装配层。
- Durable、Ephemeral、Object 是三类 Storage；PostgreSQL 只是 Durable 的官方 adapter。

## Evidence And Stop

- 依赖调整至少检查 `cargo metadata` 和 `control-plane-postgres-tests --test dependency_boundaries`。
- 若新依赖要求 contract 调用具体实现、复制 canonical DTO、改变 API/schema/权限/runtime 语义，停止并返回架构 Root 重新定界。
