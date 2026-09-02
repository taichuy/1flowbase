---
memory_type: feedback
feedback_category: repository
topic: plugin-additive-schema-single-declaration
summary: 插件增加已有表字段时只由插件 manifest 声明；目标表不再维护 extension_field_slot allowlist，Host 使用统一全局 additive schema 规则治理。
keywords:
  - plugin-data-model
  - additive-schema
  - extension-field
  - single-source-of-truth
match_when:
  - 讨论插件建表、给已有表增加字段、schema contribution、插件数据协议或扩展字段治理时
created_at: 2026-08-28 02
updated_at: 2026-08-28 02
last_verified_at: 2026-08-28 02
decision_policy: direct_reference
scope:
  - api
  - api/plugins
  - 1flowbase-official-plugins
---

# 插件 additive schema 单一声明

## 时间

`2026-08-28 02`

## 规则

插件希望向已有物理表增加字段时，只在插件自己的 schema contribution 中声明目标表与字段。不要要求目标表 owner 再维护 `extension_field_slot`、类型 allowlist 或每插件字段数配置；双重声明会形成两个事实来源并阻碍插件扩展。

Host 仍以统一全局规则治理 additive reconcile：权限、字段物理 namespace、字段 ownership ledger、允许类型、nullable/constraint 边界、容量与锁表预检、数据源 capability、安装/升级/卸载生命周期。插件只能写自己拥有的扩展字段；读取或修改 Core 领域状态继续通过领域 API、typed Port、Command 或 Event contract。

## 原因

目标表声明和插件声明同时维护，会产生漂移、协调成本和不必要的 owner 耦合。插件 manifest 应是扩展字段 desired state 的唯一来源，Host 根据实际 schema 和全局治理规则生成 Effective Schema。

## 适用场景

- 设计 Plugin Managed Data Model、schema reconcile 和插件 SDK。
- 判断新增表字段是否需要目标表逐项 opt-in。
- 编写相关 AGENTS、ADR、Issue AC 和 controlled-negative fixture。
