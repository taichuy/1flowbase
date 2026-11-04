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
node scripts/node/test-backend.js --package api-server --test api_config
node scripts/node/test-backend.js --package api-server --test api_config --filter parse_bind_addr
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
