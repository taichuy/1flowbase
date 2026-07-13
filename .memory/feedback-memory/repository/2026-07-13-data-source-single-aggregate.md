---
memory_type: feedback
feedback_category: repository
topic: 数据源应作为单一聚合统一呈现
summary: 主数据源与其他数据源属于同一个 Data Source 聚合；其他数据源实例由 RuntimeExtension 声明配置 schema，普通配置与敏感凭据分离，凭据加密落库并仅在服务端运行前解密。
keywords:
  - data source
  - main source
  - external connection
  - aggregate
  - settings
match_when:
  - 设计或调整数据源领域对象、API 或设置页信息架构
  - 讨论主数据源、其他数据源、连接实例与远端资源的层级
created_at: 2026-07-13 15
updated_at: 2026-07-14 01
last_verified_at: 2026-07-13 15
decision_policy: direct_reference
scope:
  - api/crates/control-plane/src/data_source
  - api/apps/api-server/src/routes/plugins_and_models/data_sources.rs
  - web/app/src/features/settings/components/data-models
  - web/app/src/features/settings/pages/settings-page/SettingsDataModelsSection.tsx
---

# 数据源应作为单一聚合统一呈现

## 时间

`2026-07-13 15`

## 规则

用户可理解和管理的一级对象统一为 `Data Source`。主数据源与其他数据源是同一聚合的不同后端实现，不拆成两个领域入口或两个管理区块。统一范围包括公共 identity、列表、详情入口、状态、capability 和运行时消费抽象。主数据源由 Host/Core 在插件系统之前启动并绑定 durable store；其他数据源背后的 RuntimeExtension installation、assignment 和连接实例属于数据源内部后端绑定及配置链路，只有进入具体数据源配置时才暴露必要配置。

## 原因

把领域实现差异直接投射为“主数据源”和“外部连接”两套一级对象，会让用户误以为它们不是同一种资源，并把连接实例这一基础设施对象提升成产品概念。反过来，把主数据源也强制建成依赖 plugin installation 的 RuntimeExtension 会形成启动循环，并错误放大其权限与迁移边界。统一聚合加内部后端多态可以同时保留扩展性、不同生命周期和一致管理体验。

## 适用场景

设计数据源列表、详情、创建、配置、状态、远端资源发现与 Data Model 映射时命中。后端应提供数据源统一资源与明确后端类型/能力，前端不自行拼接两个真值接口；主数据源与 RuntimeExtension 数据源分别由内部 backend adapter 承担生命周期，类型特有字段留在对应 binding/detail 中，避免伪字段和空字段。

## RuntimeExtension 实例配置与凭据

其他数据源插件声明宿主可渲染的配置 schema，每个实例把非敏感配置保存为不透明 JSON；宿主必须按 schema 校验必填项、拒绝未声明字段，并根据 `send_mode=secret_ref` 重新分类，不能信任前端把字段放入哪个 JSON 容器。声明为敏感的顶层字段只进入凭据存储，不进入公开配置；需要保留结构的嵌套 header / credential 值才使用 secret marker。

连接密码、API token、client secret 等凭据必须使用宿主主密钥加密落库，只允许服务端在调用对应 RuntimeExtension 前解密，不返回前端、不写日志或审计明文。没有凭据的数据源不创建空 secret 记录，也不伪造 `secret_ref` / `secret_version`。历史明文兼容只用于保持旧实例可读，不得在普通功能 issue 中静默重写；原地批量加密需要独立 migration 决策。
