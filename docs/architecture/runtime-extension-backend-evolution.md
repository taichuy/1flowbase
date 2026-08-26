# Runtime Extension Backend 演进边界

## 当前事实

开源版只有一个 Backend 容器和一个 `api-server` 主进程。`api-server` 是
composition root，并且只创建一个进程内 `RuntimeExtensionHost`。Host 独占活跃的
Provider、DataSource、Capability、Network Egress Registry，Worker 进程、framed
stdio、Runtime Profile 与生命周期状态。

Control Plane 与 Orchestration 不得创建具体 Host。Orchestration 先完成 target
选择，再通过 `runtime-core` 拥有的 `RuntimeExecutionPort` 执行，通过
`RuntimeObservationPort` 观察。`RuntimeBackendSlot` 的基数固定为
`exactly_one`；开源版唯一 binding 是进程内 Host。

## 稳定 Port

`RuntimeExecutionPort` 只接受强类型 `RuntimeExecutionRequest`，并提供：

- `execute`：一次请求、一次终态结果；
- `execute_stream`：同一请求的 required 与 diagnostic 事件流；
- `cancel`：按 `request_id` 取消 Host 管理的活跃任务。

`RuntimeObservationPort::snapshot` 返回生命周期、Registry 数量和活跃请求数。
Port 不暴露 HTTP/gRPC、端口、进程、文件路径、Registry 或无边界 JSON 万能调用。

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
因此修改。远程实现若需要服务发现、重试、熔断、租约、选主、调度或扩缩容，必须
另立 contract 和 Delivery；不得把这些概念提前泄漏进当前稳定 Port。

停止条件是出现第二个可工作的 Backend、第二套 Runtime Profile 真值、Port 泄漏
传输/进程细节、插件 wire 被迫变化，或实现开始承担路由、权限、事务与安装决策。
