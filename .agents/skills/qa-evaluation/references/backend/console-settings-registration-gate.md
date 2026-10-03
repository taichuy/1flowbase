# Console Settings Registration Gate

## Scope

命中后台注册设置项、Settings API、角色授权、HostExtension console contribution、注册 CLI、compiled inventory 或旧 `settings_route.visible.*` contract 替换时使用。

## Required Evidence

| 维度 | 必须证明 | 不足以通过 |
| --- | --- | --- |
| Registry ownership | 每个 Settings API 的 `method + path` 在启动注册结果中恰好归属一个 active `feature_id` | 只扫描源码、只核对路径前缀、只证明全局 middleware 已挂载 |
| Authorization | 无operation授权直接调用返回403；授权该operation后可用，同feature其他未授权operation仍拒绝；inactive feature再次拒绝 | 只证明页面隐藏或前端路由守卫生效 |
| Operation policy | 可配置operation与method/route一一对应；group enabled与full/custom独立持久化，关闭不清空custom；只有Authenticated可聚合routes | 只证明feature注册或页面开关，未验证单接口有效权限 |
| Interface descriptions | 每条console method/route有API owner提供的独立静态英文summary/description；operation无UI label_ref/description_ref | 只检查角色UI标签，未核对compiled interface catalog |
| Data boundary | operation policy 不绕过 workspace / system、owner、row、field、secret 和状态约束 | API 返回 2xx，但没有验证数据内容和越权反例 |
| Extension lifecycle | HostExtension 启停、升级、缺失版本时 surface、API 和历史 grant 行为符合 contract | 只测试 Core 内置注册项 |
| CLI determinism | CLI scaffold / update 可重复执行，输出稳定，并能检测缺失、重复和权限扩张 | 手工样例通过，未验证 CLI fixture 与错误输入 |
| Contract replacement | 功能进入过已发布开源版本时，按外部部署可能已有 grant 处理；每个受支持旧 schema/fixture 都提供逐角色 preview/delta，迁移后没有运行时双读、legacy alias 或 fallback | 以内部无人使用推断无历史数据，或只比较 permission row 数量 |

## Gate Rules

- 优先消费 Rust 启动注册产生的 compiled inventory；Node 工具负责稳定报告和 CI 结算，不用 regex 重建 Axum 真值。
- 至少提供 Core 与 HostExtension 各一个确定性 fixture，并覆盖未注册 API、重复 owner、无 grant、有 grant、inactive extension 和数据越权反例。
- 新 API 加入已有 feature 是权限扩张；inventory diff 必须明确列出 owner、旧/新 routes与operations，并分别列出full/custom角色有效权限变化。
- 正式入口为 `node scripts/node/verify.js console-operation-registry-hygiene`（直接入口见 `scripts/node/console-operation-registry-hygiene/README.md`）。它串行执行 `migrated_assembly_contains_every_console_router_owner_assembly` 与 `console_route_assembly`，默认再运行 Rust `console_operation_inventory` exporter，以当前 compiled inventory 与 baseline 结算 ownership、接口描述、migration 和权限扩张 diff；source scan 只作 advisory warning。
- compiled gate 含 Rust 测试与 exporter，按集中 Test Batch 的资源边界执行；传入 fixture inventory 也不能免除 compiled assembly 检查。报告统一落到 `tmp/test-governance/console-operation-registry-hygiene.{json,md}`。
- compiled inventory 与 Node fixture 不能单独结算真实鉴权。结合 `api/apps/api-server/tests/console_core_registry_tests.rs` 的 compiled owner / interface 证据、`api/apps/api-server/src/middleware/require_settings_feature_permission.rs` 的授权反例，并对本次受影响真实 route 补齐无授权 403、有授权可用、同 feature 其他 operation 拒绝与 inactive owner 拒绝的适用证据；未执行的授权场景仍标为未验证。
- Dev Acceptance Gate 跑最小 registry、授权和 CLI fixture；workspace cargo、按 contract 需要的 PostgreSQL 集成验证与全仓 hygiene 默认交 CI / beta。
- 无法取得 compiled inventory、鉴权反例或适用的 contract replacement 证据时写 `未验证，不下确定结论`；QA 不自动补授权、改映射、制造兼容或执行语义级修复。
