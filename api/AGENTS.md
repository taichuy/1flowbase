# Scope

- 作用域：`api/` 及其子目录。
- 下述路径默认相对 `api/`。
- 角色可配置 console operation 必须保持 `1 operation ↔ 1 method + route template`；group 的 `enabled` 与 `full/custom` 独立持久化，关闭不得清空自定义 operation。只有 `Authenticated` 可聚合 routes。
- Console operation 只承载授权语义，不得包含 UI `label_ref / description_ref`。每个 `/api/console/*` 的 `method + route template` 必须由 API owner 提供一组独立静态英文 `summary + description`，编译进全量 interface catalog；角色 UI 只消费其中可配置接口投影。

## Skills

- 做后端实现、接口、状态流转、分层边界时：使用 `backend-development`。
- 做质量评估、回归审计时：使用 `qa-evaluation`。
- 后端实现判断、新增资源模板和回归门禁不在本文件展开，分别由上述 skill 承载。

## Directory Rules

按 `api/` 目录树顺序阅读和维护：

- `apps/api-server` 是 Axum HTTP API 宿主，负责 public / console / runtime route、middleware、response、OpenAPI、loader、policy、inventory、infra bootstrap、route mount 与 boot assembly。
- `apps/api-server` 是唯一 Backend composition root，在进程内创建、注入并关闭唯一 `RuntimeExtensionHost`；不得恢复独立 Runtime 服务或端口。
- `crates/` 承载协议无关的调用内核、业务、执行与存储模块；完整 crate owner、允许依赖与代码归属只维护在 [crates/AGENTS.md](crates/AGENTS.md)，修改库代码前读取该文件及对应局部规则。
- 总体调用关系：Protocol Adapter → `interface-runtime` 冻结计划 → typed Handler → Business / Execution；这是调用关系，不授予 crate 反向依赖实现层的权限。
- `plugins` 是插件源码工作区和包工作区；`host-extensions`、`sets`、`templates`、`packages`、`installed` 的生命周期以 `api/plugins/README.md` 为准。
- `target` 是构建产物目录，不手工修改。
- 模块级与单元测试放到对应 `src/_tests`；应用宿主级健康检查、启动冒烟、跨 crate 集成验证放到 `tests/`。
- 同一目录文件接近 `15` 个时收纳子目录；单文件接近 `1500` 行时拆职责。

## Interface Lifecycle Boundary

- 架构解释见 [请求架构与调用生命周期](../docs/architecture/interface-lifecycle/README.md)；本文件维护宿主边界，Kernel 不变量见 [interface-runtime/AGENTS.md](crates/interface-runtime/AGENTS.md)。验收范围以对应候选证据为准。
- 外部业务入口统一进入 Canonical Interface；Protocol / Operational Control 显式分类，不冒充业务调用。Internal / Background Worker 的接入集合与 durable retry/ack 由各自 owner 明确，不能推导为已全量接入。
- `external_route_assembly` 保持实际 HTTP mount 与 Endpoint Catalog 同源；业务入口缺 Binding、未分类、重复或无实际 mount 时拒绝发布，不能另建手写清单掩盖裸路由。
- Composition Root 将 Effective Graph 声明、激活的认证 factory 和 typed handlers 编译为 Registry snapshot；Router、Catalog、OpenAPI 与 MCP discovery 消费相应投影，请求期间不动态拼接路由或替换计划。
- HTTP / SSE / WebSocket / MCP / WebMCP 的协议适配器保留原协议契约；WebMCP 外层调用有独立生命周期，下游业务调用只通过 lineage 关联，不能代替外层收尾。
- BuiltIn / 可信 HostExtension Authentication factory 独占原始 credential；成功后传递 sealed Principal，拒绝时保留安全的认证 attempt 关联，不向 Handler、Receipt 或普通插件传递凭证原文。
- 宿主流式调用 owner 必须保持并等待 `InterfaceStreamCompletion::complete()`；socket 关闭、投影/写入失败或桥接任务 abort 不能丢掉唯一收尾 owner。保留后台收尾任务不等于已确认响应交付。
- Invocation terminal、业务 commit/rollback、协议 delivery/ack 是独立维度：Kernel 记录调用结果，事务 owner 保证业务变更与所需 Outbox fact 原子性，协议适配器记录投影结果；连接关闭不自动发起业务取消，Completion 不冒充 subscriber ACK。

## Local Truths

- 后端验证在同一 worktree 同时只运行一条 Cargo 命令；单条命令内部默认使用机器全部逻辑 CPU 并行编译和测试，不把 `CARGO_BUILD_JOBS=1/4` 或 `--test-threads=1` 写死进开发命令或仓库配置。
- `apps/api-server/src/routes` 的协议适配器负责参数解析、认证接入和响应投影；业务调用通过已注册 Binding 进入统一 Kernel，再由 typed Handler 调用 service / action，不从协议入口直接绕过调用计划。
- API DTO 字段名优先跟领域模型 / 持久化语义一致；不要为了前端展示创建新的语义别名字段。
- `apps/api-server/src/middleware` 是请求链路约束层。
- 后台注册设置项是后端安全对象：稳定 `feature_id` 同时拥有 console surface 与 Settings API scope；前端只消费注册结果，不定义权限真值。
- SettingsFeature 是后台设置分组；组内角色授权使用单接口 operation，不得恢复为整组 feature grant 或聚合 routes。
- Settings API 未注册、重复归属或 owner inactive 时 fail closed；不得按前端 URL 推断、allow-by-default，或建立管理员可编辑的 route-to-permission 映射。
- 新增或调整后台设置注册必须使用统一 CLI 与 compiled inventory。统一入口尚未落地时，只能在已批准的 registry foundation Issue 内建立它，不新增平行手写注册表。
- 后台设置授权只解决入口与操作资格；workspace / system、owner、row、field、secret 和状态约束继续由 `control-plane` 与 repository 执行。
- controller / routes 不得直接导入具体 PostgreSQL adapter 或 `runtime-extension-host` 内部模块，通过 typed Handler 消费业务 service，协议适配器消费稳定调用 contract。
- 主仓 durable 后端官方支持 PostgreSQL；外部数据库、SaaS、API 数据源走 runtime extension。
- 业务文件二进制走 `storage-object`；插件安装包和业务文件属于不同存储域。
- 默认本地业务文件根目录是 `api/storage`；`rustfs` driver 内建但不默认启用。
- `file_storages` 是 `root/system` 资源；`workspace` 创建和消费可见 `file_tables`。
- 存储配置与文件表存储绑定归 `root/system` 管理。
- 文件记录保存实际 `storage_id`；文件表改绑只影响后续新上传。
- session 显式持有 `tenant_id` 与 `current_workspace_id`。
- 登录结果、session 读取与请求中间件继续向下传递 `current_workspace_id`。
- 单个请求链路落在一个显式 `workspace` 上下文。
- `root/system` 与业务 `workspace` 是不同命名面；外部接口与业务语义统一使用 `workspace`。
- 数据建模定义的 `scope_kind` 是 `workspace` 或 `system`；`system` 使用 `SYSTEM_SCOPE_ID`。
- runtime 物理 scope 列统一使用 `scope_id`；不使用 `team/app` alias，也不使用 `team_id/app_id` 表示 scope。
- Application 领域统一使用 `application_id`；不新增 `app_id` 缩写。
- `Boot Core` 负责启动、加载、deployment policy、root/system bootstrap、extension inventory、health/reconcile。
- `HostExtension` 是 system/root 级可信 host 模块，可定义、替换、增强 host contract；v1 是 trusted native in-process、boot-time activated、restart-scoped。
- `RuntimeExtension` 实现已注册 runtime slot，例如 `model_provider`、`data_source`、`file_processor`。
- `CapabilityPlugin` 贡献 workspace 用户显式选择的能力，例如 canvas node、tool、trigger、publisher。
- `provider`、`data source`、`file processor` 不是插件主类型，分别是 runtime slot 或 host capability。
- `storage-durable`、`storage-ephemeral`、`storage-object` 是 host contract / implementation kind。
- `storage-ephemeral`、`cache-store`、`distributed-lock`、`event-bus`、`task-queue`、`rate-limit-store` 是宿主基础设施 contract；Redis、NATS、RabbitMQ 等实现是 HostExtension provider。
- `API_EPHEMERAL_BACKEND=redis` 不是目标架构；Core 不通过 env 分支直接选择 Redis session store。
- data-source runtime extension 负责配置校验、连接测试、catalog/schema 发现、预览读取和导入快照输出；权限、secret、preview session、import job 与落盘由宿主和 `data-source-platform` 编排。
