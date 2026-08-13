---
memory_type: feedback
feedback_category: repository
topic: Catalog 依赖跟随主应用依赖唯一真值
summary: 主应用已直接使用的前端包开放给低代码 Catalog 时，Catalog 不得独立锁定另一套包版本；构建、module_version、资产和 dependency lock 必须从主应用实际解析版本派生。
keywords:
  - Catalog dependency
  - host dependency
  - single source of truth
  - module_version
  - dependency lock
match_when:
  - 主应用依赖同时作为 Frontstage 低代码开放包
  - 升级 Ant Design、icons、React 或其他 host/Catalog 共享依赖
  - 调整 official browser assets 的版本来源
created_at: 2026-08-13 09
updated_at: 2026-08-13 09
last_verified_at: 2026-08-13 09
decision_policy: direct_reference
scope:
  - web/app/package.json
  - web/packages/*-catalog
  - web/scripts/build-official-browser-assets.mjs
  - api/plugins/capability-plugins/1flowbase
---

# Catalog 依赖跟随主应用依赖唯一真值

## 时间

`2026-08-13 09`

## 规则

主应用已经拥有并直接消费的依赖若同时开放给低代码 Catalog，主应用依赖及 lockfile 的实际解析结果是唯一版本真值。Catalog 构建应使用主应用解析到的同一包，`module_version`、manifest、digest、SHA 和运行时 dependency lock 从该结果生成，不再由 Catalog package 独立选择另一版本。

## 原因

主应用与 Catalog 分别锁定版本会形成两棵依赖解析树，使宿主组件行为和低代码区块行为长期分叉。版本隔离不能替代公开 export 的向后兼容验证；升级安全应由生成门禁、真实 Runtime 渲染和 export contract 负责。

## 适用场景

- 主应用与低代码区块共同使用 `@ant-design/icons`、Ant Design、React 等依赖。
- 升级共享前端包并重新生成官方 browser assets。
- 设计 Catalog module descriptor、dependency lock 和缓存身份。
