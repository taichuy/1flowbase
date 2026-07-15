# Scope
- 作用域：`api/` 及其子目录。
- 下述路径默认相对 `api/`。

## Skills
- 做后端实现、接口、状态流转、分层边界时：使用 `backend-development`。
- 做质量评估、回归审计时：使用 `qa-evaluation`。
- 后端实现判断、新增资源模板和回归门禁不在本文件展开，分别由上述 skill 承载。

## Directory Rules
按 `api/` 目录树顺序阅读和维护：

- `apps/api-server` 是 Axum HTTP API 宿主，负责 public / console / runtime route、middleware、response、OpenAPI、loader、policy、inventory、infra bootstrap、route mount 与 boot assembly。
- `apps/plugin-runner` 是 RuntimeExtension 运行宿主，不承载控制面业务逻辑。
- `crates/access-control` 放权限目录、内建角色、权限校验。
- `crates/control-plane` 放业务 service、状态写入口、审计入口、repository trait 与外部端口。
- `crates/domain` 放领域模型、作用域语义、稳定核心对象。
- `crates/observability` 放日志、trace 与可观测性基础能力。
- `crates/orchestration-runtime` 放编排编译、绑定运行时、执行引擎、预览执行器。
- `crates/plugin-framework` 放插件 manifest / schema / contribution / registry / package 边界。
- `crates/publish-gateway` 放发布网关边界。
- `crates/runtime-core` 放 runtime registry、runtime CRUD 核心和 slot engine。
- `crates/runtime-profile` 放运行目标、locale、profile fingerprint 与插件运行环境快照。
- `crates/storage-durable` 放平台主存储边界、主存储启动入口与健康检查入口；宿主只消费这里暴露的稳定入口。
- `crates/storage-durable/postgres` 是 `storage-postgres` crate，放 PostgreSQL repository impl、查询、事务、migrations、存储层 mapper。
- `crates/storage-ephemeral` 放非持久 session store、短期协同原语与 ephemeral backend 适配。
- `crates/storage-object` 放业务文件对象存储 driver 边界；内建 `local` 与 `rustfs` driver。
- `plugins` 是插件源码工作区和包工作区；`host-extensions`、`sets`、`templates`、`packages`、`installed` 的生命周期以 `api/plugins/README.md` 为准。
- `target` 是构建产物目录，不手工修改。
- 模块级与单元测试放到对应 `src/_tests`；应用宿主级健康检查、启动冒烟、跨 crate 集成验证放到 `tests/`。
- 同一目录文件接近 `15` 个时收纳子目录；单文件接近 `1500` 行时拆职责。

## Local Truths
- 后端验证在同一 worktree 同时只运行一条 Cargo 命令；命令内部默认通过 `scripts/node/testing/verify-runtime.js cargo-jobs` 读取机器逻辑 CPU 的一半并行编译，不把 `CARGO_BUILD_JOBS=1/4` 写死进开发命令或仓库配置。
- `apps/api-server/src/routes` 是协议层：参数解析、上下文提取、调用 service / action、响应与错误映射、OpenAPI 暴露。
- API DTO 字段名优先跟领域模型 / 持久化语义一致；不要为了前端展示创建新的语义别名字段。
- `apps/api-server/src/middleware` 是请求链路约束层。
- 后台注册设置项是后端安全对象：稳定 `feature_id` 同时拥有 console surface 与 Settings API scope；前端只消费注册结果，不定义权限真值。
- 角色只授权后台设置 `feature_id`；Settings API 在 Core 启动或 HostExtension 加载时绑定到唯一 feature，不把 API action 展开成第二份角色授权。
- Settings API 未注册、重复归属或 owner inactive 时 fail closed；不得按前端 URL 推断、allow-by-default，或建立管理员可编辑的 route-to-permission 映射。
- 新增或调整后台设置注册必须使用统一 CLI 与 compiled inventory。统一入口尚未落地时，只能在已批准的 registry foundation Issue 内建立它，不新增平行手写注册表。
- 后台设置授权只解决入口与操作资格；workspace / system、owner、row、field、secret 和状态约束继续由 `control-plane` 与 repository 执行。
- `crates/control-plane` 是业务边界；关键写动作从命名明确的 service command 或 `Resource Action Kernel` action 进入。
- `crates/control-plane/src/ports` 定义 repository trait 与外部端口。
- `crates/storage-durable/postgres/src/**/*_repository.rs` 和 `crates/storage-ephemeral/src/*` 是存储或短期协同端口实现。
- actor / scope 过滤型查询属于持久化查询职责；状态流转、权限决策、审计写入属于 `control-plane`。
- `crates/storage-durable/postgres/src/mappers` 是存储模型与领域模型转换层。
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
