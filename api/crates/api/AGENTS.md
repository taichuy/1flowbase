# Scope

- 本目录承载 HTTP 宿主内核和六个协议适配域；`api-server` 保持唯一进程内 composition root。
- 不创建独立 Runtime 服务、端口或第二套启动装配。

# Ownership

| Crate | Responsibility |
| --- | --- |
| `api-host-kernel` | `ApiState`、HTTP 错误与响应、配置、中间件、认证激活、宿主基础设施、接口契约与稳定共享句柄 |
| `api-public-gateway` | 应用公开 API、兼容协议、SSE / WebSocket 与 Agent 日志协议 |
| `api-applications` | 应用管理、编排、运行时、调试、归档与日志交换 |
| `api-console-settings` | 系统设置、备份恢复、i18n 与数据模型文档 |
| `api-mcp` | MCP 管理、协议、OAuth、WebMCP 与资源导入 |
| `api-plugins-models` | 插件、模型供应商、模型、数据源与受管服务 |
| `api-console-interaction` | 前台、身份会话、助手、网络、通知通讯、终端、文件与事件 |

# Dependency Rules

- 只有 `api-server` 可以直接消费六个域 crate；域 crate 之间不得建立 Cargo 边。
- 域可以消费 `api-host-kernel`；内核不得依赖 `api-server` 或任何域。共享类型与稳定 ports 由内核拥有，不通过域间依赖复用。
- 内核的已批准下层为 `control-plane`、`control-plane-contracts`、`domain`、`extension-contracts`、`interface-runtime`、`access-control`、`plugin-framework`、`runtime-core`、`runtime-extension-host`、`orchestration-runtime`、`runtime-profile`、`observability`、`storage-durable-postgres`、`storage-ephemeral`、`storage-object`。
- 禁止边按 Cargo resolved package 身份判断，覆盖 rename、normal / dev / build 依赖；不得用 alias 或新增例外绕过边界。
- 新增依赖仍须满足上级 crate owner 规则；第三方依赖、对外契约或业务语义变化返回 Root 定界。

# Source And Test Boundaries

- 路由注册与接口贡献留在所属域；根只合并 Router、Catalog、OpenAPI 和冻结注册结果。
- 不改 HTTP / SSE / WebSocket / MCP / WebMCP 路由、DTO、授权、状态流转或持久化语义。
- 纯模块白盒测试随 owner；整应用测试留在根。根需要的窄访问器使用 `test-support` 和 `#[doc(hidden)]`，不公开整个私有业务模块。
- 单元测试放 `src/_tests`；按职责组织子目录，单目录文件不超过 15 个。保留既有测试断言与属性。
- `git mv` 移动与引用修正分别提交；路径敏感资源、治理入口与测试挂载随 owner 同步。

# Evidence And Stop

- 依赖规则由 `control-plane-postgres-tests --test dependency_boundaries` 对实际 resolved metadata 验证，坏 fixture 须证明 rename 不能绕过。
- 尚未创建的域不作为当前成员检查；最终完整装配必须包含七个 crate。
- 出现域间 Cargo 边、内核反向依赖、对外语义变化或需要弱化断言时停止并返回 Root。
