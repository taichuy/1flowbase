---
memory_type: project
topic: RuntimeExtensionHost 单一运行真值与开源版单后端拓扑已确认
summary: 用户确认开源版由 api-server 进程内装配唯一 RuntimeExtensionHost，第三方插件继续通过受控子进程和 framed stdio 执行；完整删除不承担生产职责的 plugin-runner，固定可发布 Contract、未来 SDK 与排他 RuntimeBackend Slot，但不提前实现 SDK、Remote Adapter 或集群，并保持现有官方插件 wire contract。
keywords:
  - runtime-extension-host
  - plugin-runner
  - runtime-execution-port
  - single-backend
  - stdio
  - official-plugins
  - extension-sdk
  - runtime-backend-slot
created_at: 2026-08-26 23
updated_at: 2026-08-26 23
last_verified_at: 2026-08-26 23
decision_policy: verify_before_decision
scope:
  - api/apps/api-server
  - api/apps/plugin-runner
  - api/crates/runtime-core
  - api/crates/runtime-extension-host
  - api/crates/orchestration-runtime
  - api/crates/extension-contracts
  - api/crates/extension-package-runtime
  - api/crates/plugin-framework
  - scripts/node/dev-up
  - docker
  - /home/taichuy/git/1flowbase-official-plugins
---

# RuntimeExtensionHost 单一运行真值与开源版单后端拓扑已确认

## 谁在做什么

- Root #1893 的 Delivery #1898 已创建并作为原生子 Issue 挂载，当前为 `phase:ready`；新的开发会话将从 `beta@260e33bc9974588543247959788cb9cbba23c662` 实施。
- `api-server` 是 Composition Root，进程内只装配一个 `RuntimeExtensionHost`。
- `RuntimeExtensionHost` 唯一拥有 Runtime Registry、生命周期、Worker、进程和 Runtime Profile；第三方 executable 继续使用受控子进程与 framed stdio。
- Orchestration 只通过由稳定 runtime/core contract 层拥有的 `RuntimeExecutionPort` 调用宿主；`provider-routing` 继续归 `orchestration-runtime`。
- `extension-contracts` 是未来 SDK 的可发布协议事实来源；SDK 只封装 manifest、typed request、framed stdio worker loop、stream/error/result 与 conformance tooling，不暴露后端内部 crate。
- 启动期保留排他的 `RuntimeBackend Slot`（`exactly_one`）：开源版使用 InProcess Host，未来可信商业 `HostExtension` 可贡献 `RemoteRuntimeClusterAdapter`，但 Control Plane 和 Orchestration 不增加远程分支。

## 为什么这样做

- 当前 `api-server` 和 `plugin-runner:7801` 各自创建一套 Host 状态，形成两个运行真值。
- 开源版前期需要一个 Backend 容器和一个主进程；子进程已经提供足够的第三方代码隔离，不需要内部 HTTP 服务。

## 为什么要做

- 消除 Runtime 状态、Registry、Profile 和 Worker 生命周期的双 owner，减少 dev-up、Docker、发布和运行复杂度，同时保留未来商业集群替换 Adapter 的稳定切口。

## 硬边界

- 不保留无生产职责的 `plugin-runner` harness 或空壳模块；删除其可执行代码、workspace member、端口、配置、镜像和发布链。
- 不提前实现 HTTP/gRPC、服务发现、重试、熔断、租约或集群调度。
- #1898 只固定 SDK / Remote Adapter 的 Contract、验证和替换切口，不实际发布 SDK，也不实现 Remote Adapter、动态集群或多 Backend 真值。
- `extension-contracts`、manifest、stdio wire、错误和流式语义保持；不得要求 `/home/taichuy/git/1flowbase-official-plugins` 的现有插件为本重构改协议。
- 系统状态 API 的 `plugin_runner` 字段若属于外部 contract，必须真实投影进程内 Runtime Host，或回到 Root 正式批准 contract 变化，不能伪造远端服务状态。
- 当前 `/home/taichuy/git/1flowbase_latest/api/crates/AGENTS.md` 是未提交新增，讨论阶段保留且不覆盖。

## 截止日期

- 无。
