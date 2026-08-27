# Runtime Extension Backend 演进边界

## 当前事实

开源版只有一个 Backend 容器和一个 `api-server` 主进程。`api-server` 是
composition root，并且只创建一个进程内 `RuntimeExtensionHost`。Host 独占活跃的
Provider、DataSource、Capability、Network Egress Registry，Worker 进程、framed
stdio、Runtime Profile 与生命周期状态。

Control Plane 与 Orchestration 不得创建具体 Host。Orchestration 先完成 target
选择，再通过 `runtime-core` 拥有的 `RuntimeExecutionPort` 执行，通过
`RuntimeObservationPort` 观察。API Backend 业务路径通过 typed
`ProviderRuntimePort`、`DataSourceRuntimePort`、`CapabilityRuntimePort` 与
`NetworkEgressRuntimePort` 使用对应能力，不读取具体 Registry。六个 Port 组合成
`RuntimeBackend`，由 `RuntimeBackendSlot` 以
`exactly_one` 基数绑定；开源版唯一 binding 是进程内 Host。

## 稳定 Port

`RuntimeExecutionPort` 只接受强类型 `RuntimeExecutionRequest`，并提供：

- `execute`：一次请求、一次终态结果；
- `execute_stream`：同一请求的 required 与 diagnostic 事件流；
- `cancel`：按 `request_id` 取消 Host 管理的活跃任务。

`RuntimeObservationPort::snapshot` 返回生命周期、Registry 数量和活跃请求数。
四个业务 Port 为对应 Runtime 能力提供逐操作的 typed method，所有方法均为必实现项，
不提供默认 `UnsupportedOperation`；package 激活只接收 `RuntimeArtifactReference`。
进程内 Backend 在 composition root 注入 artifact resolver，将安装记录 ID 解析为本机
materialization；该路径不进入稳定 Port。
Port 不暴露 HTTP/gRPC、端口、进程、文件路径、Registry 或无边界 JSON 万能调用。

`runtime-extension-host` crate root 只公开 `RuntimeExtensionHost`、
`RuntimeArtifactResolver` 与明确批准的稳定 Facade。Provider、DataSource、Capability、
Network Egress Host、Registry、Worker、stdio、package loader 和 Process Supervisor 均为
crate 私有；需要内部状态的测试必须位于 `src/_tests`，外部测试只走 Facade 或六个 Port。

`request_id` 标识一次 Host 调用，不写入现有插件 wire。调用方必须保证同一逻辑
尝试使用可追踪的唯一值；当前 Port 不承诺重复执行幂等。若业务需要幂等，幂等键
仍由对应业务 contract 和 Control Plane 持有，不能借 `request_id` 替代。

## 生命周期与失败

确定顺序为：

```text
Discover -> Validate -> Compile Graph -> Select Backend
-> Reconcile Packages -> Activate Workers -> Ready
-> Execute -> Drain -> Stop
```

Host 在 `Starting` 完成 package reconcile 后进入 `Ready`。`Draining` 拒绝新请求并
取消仍由 Host 管理的活跃任务；`Stop` 依次停止各 Registry 的 Worker，最终进入
`Stopped`。启动、执行、取消或停止失败必须映射为稳定的
`RuntimeBackendError` 分类，不得伪造成另一个远端服务不可达。

超时属于调用方策略，取消属于 Host 执行职责。现有插件协议没有未版本化的 cancel
frame，因此本阶段只取消 Host task / worker operation。若未来协议增加协作式取消，
必须先升级 `extension-contracts` 的协议版本并保留旧插件行为。

系统状态继续保留既有 `services.plugin_runner` JSON key 以兼容 consumer，但其 service
真值为 `runtime-extension-host`。API 与 Host profile 都来自同一 PID；Host group 的
进程内存和进程数只采样一次，Host profile 失败会让状态请求失败，不再伪造远端服务
unreachable。

## Worker 与协议版本

插件继续使用已发布 manifest、execution mode 与 framed stdio：

- `process_per_call`；
- `stateful_provider_worker`；
- `stateful_runtime_worker`；
- `stdio_json` 与 `stdio_json_worker`。

Worker 状态由 Host 管理，最小状态机为启动、可用、执行、排空、停止/失败。协议的
method、event、error、result 与优雅退出语义以 `extension-contracts` 为唯一事实。
Host 不承担 Provider Routing、权限、事务、安装或签名决策。

## 未来 SDK

本阶段不发布 SDK。未来 Rust、TypeScript、Python SDK 只能建立在可发布的
`extension-contracts` 上，提供 manifest 构造/校验、强类型请求、framed stdio
worker loop、handler 注册、流事件、标准错误、优雅退出、Host Simulator、golden
fixture 与 Conformance Kit。SDK 不得暴露 Host Registry、Routing、Control Plane、
Domain、数据库或 `api-server` 内部类型，也不得打包整个后端 crate graph。

## 未来 Remote Adapter

本阶段不实现远程协议、SDK 或集群。未来可信 `HostExtension` 可以贡献
`RemoteRuntimeClusterAdapter`，但它只能替换 composition root 中
`RuntimeBackendSlot` 的唯一 binding。Control Plane、Orchestration 与业务路由不得
因此修改。远程 Adapter 解析同一 `RuntimeArtifactReference`，不能要求业务路径传递
本机 package path。远程实现若需要服务发现、重试、熔断、租约、选主、调度或扩缩容，必须
另立 contract 和 Delivery；不得把这些概念提前泄漏进当前稳定 Port。

任何 Backend 必须在编译期完整实现六个 Port。本阶段不定义部分能力 Backend；若未来
确有该需求，必须另立 Delivery，引入显式 capability set 并在 Slot bind 时校验，不能
恢复默认失败方法或在业务调用期探测能力。

官方兼容门禁必须构建并启动 8 个官方 executable，经真实 Host 验证三种 execution
mode、两种 stdio protocol、Validate、CountTokens、Generate 的 event/error/result 与
Clash stateful Network Egress worker；manifest-only 检查不能结算 Runtime 行为兼容。

停止条件是出现第二个可工作的 Backend、第二套 Runtime Profile 真值、Port 泄漏
传输/进程细节、插件 wire 被迫变化，或实现开始承担路由、权限、事务与安装决策。
