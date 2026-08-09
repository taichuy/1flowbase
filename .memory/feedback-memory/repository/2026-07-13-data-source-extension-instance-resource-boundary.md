---
memory_type: feedback
feedback_category: repository
topic: 外部数据源设计必须拆分扩展、连接实例、远端资源与 Data Model
summary: 讨论外部数据源时，不能从 Data Model 字段直接倒推绑定关系；必须区分插件扩展、安装分配、连接实例、远端资源、Data Model 与独立的 Data Model Template，并以能力包含关系组合数据源和模板。
keywords:
  - data-source
  - extension
  - connection-instance
  - external-resource
  - data-model
  - data-model-template
  - operation-registry
  - plugin-boundary
match_when:
  - 设计或调整数据源管理、外部连接和 Data Model 映射
  - 讨论 HostExtension、RuntimeExtension 与数据源实例关系
  - 设计数据源页面层级、接口 DTO 或资源绑定字段
  - 设计普通表、树状表或插件表模板及其生成接口
created_at: 2026-07-13 00
updated_at: 2026-08-10 00
last_verified_at: 2026-08-10 00
decision_policy: direct_reference
scope:
  - api/plugins
  - api/crates/control-plane/src/data_source
  - api/apps/api-server/src/routes/plugins_and_models/data_sources.rs
  - web/app/src/features/settings
  - .memory/feedback-memory/repository
---

# 外部数据源设计必须拆分扩展、连接实例、远端资源与 Data Model

## 规则

- 需求分析必须先拆分：插件扩展能力、插件安装与工作区分配、用户创建的连接实例、连接器发现的远端资源、平台 Data Model。
- “连接器提供什么能力”和“用户连接到哪一个外部系统”不是同一个对象；同一种连接器可以创建多个连接实例。
- Data Model 的远端资源映射必须经过连接实例，不能把孤立的 `table_id` 当成完整绑定，也不能从一个 UI 字段直接反推整体架构。
- `HostExtension` 与 `RuntimeExtension` 的正式类型归属必须以当前项目硬边界和已确认架构为证据，不用“Host 托管”替代插件消费类型判断。
- Data Model Template 与 DataSource 是两个独立维度；创建 Data Model 时按 `required_capabilities(template) ⊆ provided_capabilities(data_source)` 过滤可组合项，不能把模板类型硬编码成某个数据源类型。
- Host/Core 只统一拥有模板目录、兼容校验、权限、生命周期和 API/OpenAPI 注册。需要直接操作主 PostgreSQL 动态 schema 与事务的模板实现属于可信 HostExtension/Core；外部数据源自己的模板和 operation handler 可以由对应 RuntimeExtension 提供。
- 模板字段、operation、路由、输入输出 schema、权限与 handler 必须来自同一个版本化 descriptor；缺项或能力不匹配时 fail closed，不能让多个注册入口分别漂移。

## 原因

- 把扩展、实例和资源压成一层，会让安装生命周期、密钥配置、连接状态、Catalog 发现和 Data Model 映射互相泄漏。
- 主数据源是 Host/Core 内建能力，外部连接实例是用户配置对象；用同一个 instance DTO 表达两者会继续制造伪字段和错误交互。
- 把所有模板都放进 HostExtension 会错误扩大可信边界；反过来让 RuntimeExtension 直接写平台主库，又会破坏主存储与权限 owner。按数据访问能力决定实现 owner，Host 只保留统一控制面，复杂度最小。

## 适用场景

- 数据源管理页、连接创建页、远端资源 Catalog 与 Data Model 映射流程
- 数据源插件协议、安装分配、连接密钥和运行时调用
- `data_source_instance_id`、`external_resource_key`、`external_table_id` 等字段契约讨论
