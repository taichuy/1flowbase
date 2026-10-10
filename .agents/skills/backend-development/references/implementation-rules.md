# 后端实现规则

## When / Then Rules

- When 新增或修改 HTTP route，then 协议适配器通过冻结 Binding/Plan 进入 Kernel，typed Handler 后的状态变化进入 service command 或 `Resource Action Kernel` action；装配与证据见 [interface-lifecycle.md](interface-lifecycle.md)。
- When 修改 middleware，then middleware 只处理请求链路约束；不写业务状态。
- When 新增关键写动作，then 同步设计 service/action 入口、权限、审计、幂等和回归测试。
- When 修改成员、角色、权限、模型或会话关键动作，then 写审计日志。
- When 写动作影响 session 安全边界，then 经过显式 service；when 该接口需要 CSRF 保护，then 校验 `x-csrf-token`。
- When 新 Core 写动作要成为 HostExtension 扩展点，then 先进入 `Resource Action Kernel`；未进入 kernel 的 route 不是 HostExtension hook 扩展点。
- When HostExtension 实现或增强 host contract，then manifest 声明 contribution；native entrypoint 只注册已声明的 resource、action、hook、route、worker、migration 和 infrastructure provider。
- When HostExtension 启停或升级，then 写 desired state；实际激活在重启后生效；Rust native `so/dll` 热卸载不是 v1 目标。
- When HostExtension 写 migration，then 使用 `ext_<normalized_extension_id>__*` 命名空间；不修改 Core 真值表。
- When pre-state infra provider bootstrap 运行，then 它发生在 `ApiState`、session store、control-plane service、runtime engine 和 HTTP router 构造前。
- When workspace / tenant 消费宿主能力，then 只配置、绑定或消费宿主已安装能力。
- When 受管插件声明贡献或选择激活作用域，then 按[插件组合](../../../../docs/architecture/plugin-composition.md)核对现有注册、绑定与执行链路；旧 workspace/model 分配限制是待演进的实现事实，不是永久选型规则。
- When 受管插件贡献数据、页面或业务接口，then 由宿主已开放的 typed contract 注册、授权和执行；未开放时报告契约缺口，不自行挂载路由、接入认证 factory 或直接写平台主存储。
- When runtime 模型或字段缺少物理表 / 列，then 标记不可用；不健康元数据不进入 runtime registry。
- When data-source plugin 接入外部数据库、SaaS 或 API，then 它走受管数据源贡献；HTTP 注册与 OAuth callback 由宿主契约承接，不自行挂载接口、不直接写平台数据库。
- When 命名 storage 边界，then 保持 `storage-durable`、`storage-ephemeral`、`storage-object`；不改名为 cache，不新增 `Driver` 层级。
- When 需要存储层结构转换，then 新增 mapper；否则不要为凑结构拆空文件。
- When 新增测试，then 放入对应 `_tests` 子目录。

## 新增关键写资源最低形态

When 新增关键写资源，then 至少包含：

- `apps/api-server/src/routes/<resource>.rs`
- `crates/control-plane/src/<resource>.rs` 或 `crates/control-plane/src/<resource>/mod.rs`
- `crates/control-plane-contracts/src/ports/<resource>.rs` 中对应的 adapter-facing repository trait；业务端口仍由 `control-plane` 持有
- `crates/storage/durable/postgres/src/<resource>_repository.rs` 或 `crates/storage/ephemeral/src/<resource>_repository.rs`
- 对应 `_tests`

`dto` 可定义在 route 模块内。只有存在存储结构转换时才新增 mapper。`crates/storage/durable/postgres/migrations` 只放数据库迁移；路径与 port owner 以 `api/crates/AGENTS.md` 为准。
