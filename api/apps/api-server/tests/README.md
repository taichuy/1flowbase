# API test targets / API 测试目标

Use a Cargo target to select what is compiled, then a test-name filter to select what runs.
`--lib <filter>` still compiles the whole library test harness. The repository CLI already
supports `--test`; no additional test runner is required.

先选择 Cargo 编译目标，再按需过滤测试名。`--lib` 下的名称过滤仍需编译整个库测试程序。
仓库测试 CLI 已支持 `--test`，直接复用既有资源预算、退出码与非零执行检查。

| Cargo target | Scope / 覆盖范围 | Test-name prefix / 测试名前缀 |
| --- | --- | --- |
| `api_config` | API configuration and bind address / 配置与监听地址 | `config_tests::` |
| `http_error_contract` | Public HTTP error mapping / 公开 HTTP 错误映射 | `error_response_tests::` |
| `extension_graph_contract` | Extension graph input assembly / 扩展图输入装配 | `input_assembly_tests::` |
| `host_registration_contract` | Builtin extensions, routes and workers / 内置扩展、路由与 worker 注册 | `host_extensions_builtin_tests::`, `host_route_registry_tests::`, `host_worker_registry_tests::` |
| `interface_coverage_inventory` | Interface inventory contract / 接口覆盖清单契约 | `interface_coverage_inventory::` |

From the repository root / 在仓库根目录执行：

```bash
node scripts/node/test-backend.js --workspace --test api_config
node scripts/node/test-backend.js --workspace --test api_config --filter parse_bind_addr
```

Each entry mounts the original test leaf under `src/_tests` and links the ordinary
`api_server` library. The source files stay in place so relative fixture/include paths
keep their meaning. Those leaves are no longer mounted by the library test harness.
Private implementation tests remain in their owning modules.

每个入口仅挂载 `src/_tests` 中的原测试文件，并链接普通 `api_server` 库。
原文件不移动，保留相对 fixture/include 路径；原库测试入口不再重复挂载这些文件。
需要访问私有实现的白盒测试仍留在所属模块。

Full package/workspace test selection still discovers these targets and the remaining
library tests. Keep the existing feature-specific and documentation-test gates;
`--all-targets` does not mean every possible feature combination. The
`root-1805-consumer-fixture` feature is a narrow library fixture, not the default suite.

完整 package/workspace 测试仍发现这些目标和剩余库单测；保留既有专项 feature 与文档测试门禁。
`--all-targets` 不代表所有 feature 组合。`root-1805-consumer-fixture` 是窄范围库测试 fixture，
不能代替默认全集。

`reset-rust-cache/cli.js --warmup-only` precompiles the full selected workspace test
scope while preserving existing caches and services. It does not execute tests.
Run the test CLI for behavioral evidence; a zero-test success is not accepted.

`reset-rust-cache/cli.js --warmup-only` 保留缓存和服务，预编译所选 workspace 的全部测试目标；
它不执行测试。行为验证使用测试 CLI，并继续拒绝零用例成功。

For these extracted targets, prefer explicit `--workspace --test <target>` to reuse the
workspace warmup feature graph while selecting only the named test target. The
[2026-11-04 verification](../../../../docs/operations/rust-test-resource-verification-20261104.md)
compared the ordinary `--workspace --test api_config` graph, with no test-debug
environment override, against workspace `--all-targets`: all 570 recursive units
were present in the 650-unit warmup graph with matching effective profiles and
dependency edges. Package-only selection previously produced a different feature
graph and rebuilt dependencies. This evidence is specific to the measured source,
Cargo version and `api_config` target; recheck after relevant dependency changes.
`--workspace` and `--package`/`-p` are mutually exclusive; package selection remains available.

这些独立目标优先用显式 `--workspace --test <target>`，在只选择指定测试目标的同时复用
workspace 全量预热的 feature 图。上述验收清除额外 test-debug 环境覆盖后确认：
普通 `--workspace --test api_config` 的 570 个递归 units 全部包含在预热的 650-unit
图中，有效 profile 与依赖边差异为 0。旧单包选择曾产生不同 feature 图并重编依赖。
此结论绑定报告所列源码、Cargo 版本及 `api_config` 目标；相关依赖变化后应重新检查。
`--workspace` 与 `--package`/`-p` 互斥；原 package 选择继续可用。

## HTTP behavior partitions

The `test-support` feature is enabled by the package's dev self-dependency. It adds
explicit fixture factories; ordinary production constructors keep their behavior.
Each target below mounts original test leaves and shares one fixture library.
Private implementation cases remain in the library harness, including private contract checks extracted from otherwise public test leaves.

| Target | Scope |
| --- | --- |
| `api_application` | Application, runtime, logs and model HTTP behavior |
| `api_identity` | Authentication, members, roles and sessions |
| `api_console` | Console policy and Frontstage HTTP behavior |
| `api_system_settings` | Data sources, files and system settings |
| `api_mcp_management` | MCP management, bundles and upstreams |
| `api_documentation` | Public OpenAPI and documentation contracts |
| `api_extension_catalog` | Model providers, plugin HTTP behavior and network center |

Run `node scripts/node/test-backend.js --workspace --test api_application` from
the repository root, with an optional `--filter`. Full workspace/package tests
continue to discover every target. Executable discovery and behavioral runs remain the acceptance evidence for
complete coverage; selecting a target does not run the remaining targets.

Fixture resources remain owned by runtime services after state/provider clones.
The owner is declared last so backend/session destructors finish before cleanup.
Integration fixtures use the production UUID artifact resolver; library unit
fixtures retain their existing path resolver. `ApiState` gains no feature field.

The workspace test profile defaults to `debug = 0`, matching cache warmup.
Ordinary target selection therefore reuses the same dependency profile without an extra environment override.
The existing API package override still retains line-table debug information.

## Fixture prerequisites / Fixture 前提

`api_config` is a pure configuration target and needs no test database. HTTP targets
use `API_DATABASE_URL` or the repository's existing local test database on port
35432. Their `PostgresTestSchema` fixture creates isolated temporary databases;
the configured test role needs the existing database, schema and extension privileges,
plus access to the `postgres` maintenance database for owner-triggered cleanup.
Use the configured test database account.

`api_config` 是纯配置目标，无需测试数据库。HTTP 目标沿用 `API_DATABASE_URL` 或仓库
现有的本机 35432 测试数据库。`PostgresTestSchema` 创建独占临时数据库；测试角色需要
既有建库、schema 和扩展权限，并能连接维护库 `postgres` 进行 owner 析构清理。
清理失败会记录错误；使用测试数据库账号。

The remaining full library suite also uses the existing trusted-origin and SDK-worker
fixtures. Keep the CI fixture value `API_AUTH_PUBLIC_ORIGIN=https://auth-fixture.example`
and prepare the required worker artifacts before the cases that consume them. The
repository's `scripts/node/plugin-composition-test-batch/runner.js` owns the existing
plugin-composition worker preparation; the verification report records the four worker
examples used for the full library run. Let Cargo set `CARGO_MANIFEST_DIR` for each
package instead of exporting one application path globally.

完整剩余库回归还依赖既有 trusted-origin 与 SDK worker fixture。沿用上述 CI origin，
在相应用例执行前准备 worker 产物；既有插件组合 runner 管理其 worker 准备，完整库
验收使用的四个 example 见报告。普通 Cargo 命令由 Cargo 为各包设置
`CARGO_MANIFEST_DIR`，不要把某个应用包目录全局导出给所有依赖。

`--warmup-only` runs the same four warmup phases as the default reset entry: locked
fetch, dev API binary build, workspace binary-test compilation, then workspace
all-target compilation. It skips the default service-stop and cache-deletion steps.
Cold dependencies and the ordinary API library can still require substantial memory;
selecting a small test target does not remove those prerequisites.

`--warmup-only` 与默认 reset 入口共用 fetch、dev API binary、workspace binary tests、
workspace all-targets 四个预热阶段，跳过默认停服和删缓存步骤。冷依赖与普通 API 库
仍可能占用较多内存；选择较小测试目标不会免除这些前置编译。
