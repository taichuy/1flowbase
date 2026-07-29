---
memory_type: project
topic: 认证器协议驱动的公开认证区块与自行注册
summary: 用户确认代码区块是认证器正常公开 UI 的唯一真值，Core 登录页只做实例选择与共享 Block 宿主；已启用的内置账号密码认证新增 Block 故障时的 Core 紧急登录表单，#1444 继续处于用户验收。
keywords:
  - auth center
  - authenticator
  - public ui block
  - self registration
  - password-local
  - schema ui
  - ctx.api
match_when:
  - 继续规划或实现认证中心登录、注册、扫码或第三方认证 UI
  - 调整认证器 public projection、HostExtension Auth Provider contract 或注册接口
  - 讨论登录页应由 Core、Schema UI 还是代码区块拥有
created_at: 2026-07-24 00
updated_at: 2026-07-29 10
last_verified_at: 2026-07-29 10
decision_policy: verify_before_decision
status: phase_user_acceptance
source_issue: "#1444"
related_issues:
  - "#1154"
  - "#1155"
  - "#1156"
  - "#1162"
  - "#1182"
  - "#1185"
  - "#1393"
scope:
  - api/crates/domain/src/auth
  - api/crates/control-plane/src/auth
  - api/apps/api-server/src/routes/identity/auth.rs
  - api/apps/api-server/src/routes/settings/auth_center.rs
  - api/crates/plugin-framework
  - web/app/src/features/auth
  - web/app/src/features/settings
  - web/packages/page-protocol
  - web/packages/page-runtime
---

# 认证器协议驱动的公开认证区块与自行注册

- 谁在做什么：认证中心准备把公开登录页改为通用认证器宿主；Auth Provider 注册后端配置、默认完整 Block、公开变量和 public Auth API，认证器实例保存自己的 Block。
- 为什么这样做：认证插件是后端安装物，不能依赖修改已部署 Core 前端；Core 表单、Schema UI 表单和插件 TSX 并存会产生多份 UI 真值。
- 为什么要做：需要让 password-local 登录/自行注册以及未来扫码、OIDC、SAML 等认证方式使用同一协议，并确保注册开关在浏览器被伪造时后端仍拒绝。
- 截止日期：未指定；当前仅完成线上计划建立，未进入实现。
- 决策动机：UI 复杂度属于认证器完整代码区块，安全与动作许可属于 Auth Center 后端，共享代码区块 runtime/component 只提供通用执行能力。

## 已确认决策

- 每个认证器实例只有一个完整 `public_ui_block`，登录和注册可以同时存在于同一 Block；Core 不保留第二份账号密码表单。
- 一个实例直接渲染；多个实例显示一列按钮并渲染所选 Block；零实例显示不可用状态。
- `self_registration_enabled` 等配置由后端持久化并投影为 `public_variables`，仅供 Block 决定显示；注册 API 必须重新读取后端配置并 fail closed。
- Auth 页面复用 canonical `BlockModule + main`、typed inputs、renderer/runtime 和 `ctx.api.<method>(path, request)`，只增加 Auth host adapter，不创建 Auth 专用 AST/TSX/runtime 或接口目录。
- 插件默认 Block 只用于新实例与历史空值回填；插件升级不覆盖已保存的用户 Block。
- 公开登录页从认证器 public projection 读取 Block，不依赖 Frontstage page/document/block-code API。

## 当前线上状态

- Single Issue：[#1444](https://github.com/taichuy/1flowbase/issues/1444)，阶段为 user acceptance。
- 认证中心完整共享 Studio、`/api/public/*` 连接器后端过滤及动态 `context_variables` 已依次合入 `beta`。
- 动态上下文变量交付提交：implementation `e221ec48d`，beta merge `4e0d4ace8`；AC-017～AC-021 证据见 Issue comment `5069033332`。
- Auth Provider 公开变量 schema 从既有 `config_schema + public_variable_keys` 派生；前端直接消费 `label / member_path / schema`，不维护 Auth 专用变量常量。
- 变量作者界面统一为 `标签 / 变量 / 操作` 三列表格；Provider public variable 标签复用 `config_schema` 字段标签，Auth 缺失上下文目录时 fail visible，不回退 Frontstage 通用变量。交付提交：implementation `d54d5dca8`，beta merge `44aa89fb3`。
- Auth 配置变量与运行时上下文由后端 `group` 字段分组；Host 安全投影 `title / description / enabled`，Provider 继续只投影 `public_variable_keys`，不公开 `public_ui_block`、未知字段或 secret。交付提交：implementation `42a84d16d`，beta merge `bc944e00a`。
- Auth Center authoring DTO 直接返回 registry 安全投影的 `public_variables`；认证中心运行当前未保存草稿时复用共享 Trial UI，并以同一 `authenticator_id / public_variables / auth_event` contract 重跑 action。真实写接口调用先确认，取消或 session revoke 后不发出请求；越界路径继续由 canonical public Auth transport 拒绝。交付提交：implementation `7e7d0c879`，beta merge `d1600be24`。
- Auth Studio 的运行预览使用共享 `JsBlockTrialPanel` 的 `direct-preview` presentation，右侧只展示实际 Block 渲染结果，Frontstage 继续使用完整 `debugger` presentation；初版草稿防抖刷新语义已被下一条保存真值语义取代。初版交付提交：implementation `6e9050cf4`，beta merge `3cdb76259`。
- Auth Studio 预览刷新语义曾收敛为保存真值、仅保存后刷新；该交互语义已被下一条“运行 / 保存解耦”取代。阶段交付提交：implementation `8dff95011`，beta merge `a74058c47`。
- 共享 Studio 顶部顺序为“上下文 / 重置 / 保存 / 运行”，运行是 primary 主操作：点击时冻结当前未保存草稿、切换到预览并通过显式 revision 运行；继续编辑不自动重跑。“保存”只持久化，不切换或运行；预览轨道只负责查看最近结果。Auth 使用 `direct-preview`，Frontstage 继续使用既有 `debugger` runtime。语义交付提交：implementation `496e47c30`，beta merge `df6be02b3`；顺序与主操作修正：implementation `2b090cbe0`，beta merge `0cb0e2141`。
- Auth `direct-preview` 使用 65% / 35% 的预览与控制台垂直分屏；控制台直接消费 runtime 已清洗的 `snapshot.logs`，展示 level、message 与结构化 data。中间 splitter 支持鼠标和键盘调节，两侧独立滚动；Frontstage debugger 保持原控制台能力。交付提交：implementation `4589fa9e1`，beta merge `56cc7f370`。
- Auth 内嵌控制台使用白色轻量 DevTools 风格而非深色终端：空状态只显示左侧高亮 `>` prompt，日志行使用固定 gutter，info/debug/warn/error 通过符号和颜色区分；不提供输入或 REPL。交付提交：implementation `0d4c1db14`，beta merge `cbc428360`。
- `2026-07-26 00` 用户批准普通区块与认证中心 Studio 统一：顶部仅运行，运行区固定为“实际区块预览 + 单一输出控制台”，不包含停止、多标签或调试浮窗；设置共享同一 `Descriptions` 骨架并保留领域字段与编辑性。实现已进入任务级 QA，未修改后端或 runtime contract。
- `2026-07-28 19` 用户批准内置账号密码认证锁死保护：正常路径继续以 `public_ui_block` 为唯一 UI 真值；只有后端投影为已启用、`is_builtin` 且 `auth_type=password-local` 的实例，才在编译、准备、运行时失败或 10 秒超时后自动切换 Core 最小登录表单。实现提交 `7ca5fd92e` 已合入 `beta`。
- `2026-07-29 10` 用户人工验收发现并批准修正正常态常驻的手动紧急切换：`ready` 只显示配置 Block；已启用的内置 `password-local` 在编译、准备或运行时首次失败后自动重试一次，重试仍失败或第二次 10 秒超时才进入 Core 内置表单；其他认证器保持原错误与人工重试。修复提交 `3aba07707` 已合入 `beta`，等待用户重启后再次验收。
