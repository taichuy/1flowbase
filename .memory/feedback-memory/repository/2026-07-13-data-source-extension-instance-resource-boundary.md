---
memory_type: feedback
feedback_category: repository
topic: 外部数据源设计必须拆分扩展、连接实例、远端资源与 Data Model
summary: 讨论外部数据源时，不能从 Data Model 字段直接倒推绑定关系；必须先区分提供连接能力的插件扩展、其安装与分配、用户创建的外部连接实例、连接器发现的远端资源，以及平台 Data Model。
keywords:
  - data-source
  - extension
  - connection-instance
  - external-resource
  - data-model
  - plugin-boundary
match_when:
  - 设计或调整数据源管理、外部连接和 Data Model 映射
  - 讨论 HostExtension、RuntimeExtension 与数据源实例关系
  - 设计数据源页面层级、接口 DTO 或资源绑定字段
created_at: 2026-07-13 00
updated_at: 2026-07-13 00
last_verified_at: 2026-07-13 00
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

## 原因

- 把扩展、实例和资源压成一层，会让安装生命周期、密钥配置、连接状态、Catalog 发现和 Data Model 映射互相泄漏。
- 主数据源是 Host/Core 内建能力，外部连接实例是用户配置对象；用同一个 instance DTO 表达两者会继续制造伪字段和错误交互。

## 适用场景

- 数据源管理页、连接创建页、远端资源 Catalog 与 Data Model 映射流程
- 数据源插件协议、安装分配、连接密钥和运行时调用
- `data_source_instance_id`、`external_resource_key`、`external_table_id` 等字段契约讨论
