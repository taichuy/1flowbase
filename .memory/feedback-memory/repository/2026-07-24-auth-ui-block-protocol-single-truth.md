---
memory_type: feedback
feedback_category: repository
topic: 认证插件公开 UI 必须由完整代码区块协议驱动
summary: 后端认证插件不能改 Core 前端，因此每个认证器的完整公开 Block 是唯一 UI 真值，Core 只能选择和挂载。
keywords:
  - auth-provider
  - public-ui-block
  - protocol-rendering
  - single-source-of-truth
match_when:
  - 设计认证、支付或其他后端插件安装后需要新增前端交互的扩展协议
  - 在 Schema UI、TSX 复用、代码区块和 Core 硬编码之间选择职责边界
created_at: 2026-07-24 14
updated_at: 2026-07-24 14
last_verified_at: 2026-07-24 14
decision_policy: direct_reference
scope:
  - api/crates/plugin-framework
  - api/crates/control-plane/src/auth
  - web/app/src/features/auth
  - web/packages/block-renderer
---

# 认证公开 UI 的协议边界

## 时间

`2026-07-24 14`

## 规则

后端插件安装后无法修改 Core 前端时，不得把简单登录交给 Schema UI、复杂登录交给 TSX，或保留 Core 默认表单形成两套维护路径。每个认证器实例持久化一个完整 `public_ui_block`，区块本身决定登录、注册、扫码、跳转和布局；Core 仅发现实例、选择实例、注入后端公开变量与 canonical Block context、挂载共享 renderer/runtime，并隔离错误。

认证中心的 `UI` 作者入口同样必须复用 Frontstage 代码区块的标准浮动 TSX Studio 基准，包括共享窗口、编辑器、状态与窗口动作；不得只抽 Monaco 后再包一层 Auth 专用 Drawer / Modal，形成第二套作者交互。

“复用 Studio”指复用完整共享组件及全部标准资源区：代码、接口连接器、变量、组件、配置和运行。不得只复用窗口壳、标题栏、Monaco 或裁选后的单个代码资源；Frontstage 与认证中心的差异只能通过 adapter 注入数据、接口范围、变量和保存方法。

显示变量只负责让 Block 决定界面。后端动作必须重新读取持久化配置并独立授权，浏览器篡改变量、Block 或请求不能开启注册等能力。

## 原因

认证插件属于后端安装能力，不能要求每新增一种认证器就修改或重新发布 Core 前端；双渲染体系会产生协议能力差异和长期双写。

## 适用场景

认证器、后端插件贡献公开交互、通用代码区块 host adapter、插件配置到公开变量投影，以及其他“后端扩展但 Core 前端不可随插件更新”的能力。

## 备注

允许 Auth Center 的后台配置表单继续使用后端 `config_schema`；该 schema 是后台配置入口，不是公开登录/注册 UI 的第二真值。
