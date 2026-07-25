---
memory_type: feedback
feedback_category: repository
topic: 认证 UI 复用完整前端区块协议并由后端认证器拥有内容
summary: 设计登录、注册及扫码等认证 UI 时，代码块是认证器公开 UI 的唯一真值；Core 只按认证器数量选择实例、注入后端公开变量并渲染完整区块，不生成登录/注册 UI、解释变量或依赖 Frontstage 资源模型。
keywords:
  - auth center
  - schema ui
  - TSX
  - login
  - signup
  - composition
match_when:
  - 设计登录、注册、扫码、OIDC 或其他认证页面
  - 讨论 Schema UI 与代码区块是否并存或复用
  - 设计认证插件默认 UI 模板及升级策略
created_at: 2026-07-23 23
updated_at: 2026-07-24 00
last_verified_at: 2026-07-24 00
decision_policy: direct_reference
source_issue: "#1444"
scope:
  - web/app/src/features/auth
  - web/app/src/shared/schema-ui
  - web/packages/page-runtime
  - web/packages/page-protocol
  - api/crates/control-plane/src/auth
---

# 认证 UI 使用单一组合合同

## 规则

登录、注册、扫码及其他认证方式应直接复用现有前端代码区块的完整协议、运行时能力和渲染组件，不另建认证专用受限 TSX、Auth Surface AST 或按认证类型写死的前端 renderer。

认证插件属于后端安装物：插件注册认证后端协议、认证器配置 schema 以及默认区块内容；创建认证器实例后，UI 内容存入认证器所属的后端记录并可从认证中心配置表单编辑。公开登录页从认证器 public projection 读取区块描述，交给共享前端区块组件渲染，不读取或依赖 Frontstage page/block-code API。

认证区块只继承代码区块本身已有的 runtime、module、permission 与 capability contract，不额外缩减能力。私有认证配置与公开 UI 内容必须由后端分开投影，前端不猜测或兼容字段。

代码块是认证器公开 UI 的唯一真值。Core 登录页不根据 `registration_enabled` 等变量生成、隐藏或拼装登录/注册控件，只把认证插件声明为公开的变量注入区块上下文；区块源码自行决定是否展示注册、登录、扫码或其他内容。后端继续从持久化认证器配置判定动作是否允许，浏览器修改 UI 或注入值不能改变后端决策。

登录页只处理认证器实例选择：只有一个可用认证器时直接渲染其区块，不展示选择器；多个时展示实例选择并渲染当前区块。不得另建一份 Core 登录表单与认证器区块竞争 UI 真值。

## 原因

用户连续纠正：将普通表单和复杂认证 UI 拆成不同运行时会产生双重维护；进一步明确认证插件无法修改已部署前端，因此区块是什么能力，认证 UI 就应具备什么能力。前端只提供协议驱动的通用区块组件，具体 UI、默认源码和后端动作由认证插件及认证器记录拥有。2026-07-24 用户补充，注册开关等公开配置只作为变量注入，显示逻辑完全由代码块决定，Core 不再维护第二份能力到 UI 的映射。

## 适用场景

认证中心、公开登录/注册页、认证 HostExtension、认证器配置 Schema UI、TSX 模板注入、共享前端区块组件。

## 线上计划真值

- GitHub Issue：`taichuy/1flowbase#1444`，标题“建立认证器协议驱动的公开认证区块与自行注册”。
- 当前阶段：`phase:discussion`；Issue 已创建但尚未授权实现，用户确认 Issue 合同后才进入 `phase:ready`。
