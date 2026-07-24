---
memory_type: project
topic: 认证器协议驱动的公开认证区块与自行注册
summary: 用户确认代码区块是认证器公开 UI 的唯一真值，Core 登录页只做实例选择与共享 Block 宿主；#1444 已进入实现，认证中心完整共享 Studio、公开 API 连接器过滤和动态 Auth Block 上下文变量已合入 beta。
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
updated_at: 2026-07-24 22
last_verified_at: 2026-07-24 22
decision_policy: verify_before_decision
status: phase_implementation
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

- Single Issue：[#1444](https://github.com/taichuy/1flowbase/issues/1444)，阶段为 implementation。
- 认证中心完整共享 Studio、`/api/public/*` 连接器后端过滤及动态 `context_variables` 已依次合入 `beta`。
- 动态上下文变量交付提交：implementation `e221ec48d`，beta merge `4e0d4ace8`；AC-017～AC-021 证据见 Issue comment `5069033332`。
- Auth Provider 公开变量 schema 从既有 `config_schema + public_variable_keys` 派生；前端直接消费 `label / member_path / schema`，不维护 Auth 专用变量常量。
- 变量作者界面统一为 `标签 / 变量 / 操作` 三列表格；Provider public variable 标签复用 `config_schema` 字段标签，Auth 缺失上下文目录时 fail visible，不回退 Frontstage 通用变量。交付提交：implementation `d54d5dca8`，beta merge `44aa89fb3`。
