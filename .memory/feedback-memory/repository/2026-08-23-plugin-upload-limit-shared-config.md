---
memory_type: feedback
feedback_category: repository
topic: 插件上传限制必须使用全局共享配置
summary: 用户明确要求插件上传大小不能由网络中心等单个入口私有配置；所有插件包上传路由必须共享同一个 API 配置，默认值维持 8 MiB。
keywords:
  - plugin upload
  - upload limit
  - shared configuration
  - API_PLUGIN_UPLOAD_MAX_BYTES
match_when:
  - 新增或调整插件包上传大小限制
  - 某个控制台页面需要上传配置而同类入口已存在
created_at: 2026-08-23 12
updated_at: 2026-08-23 12
last_verified_at: 2026-08-23 12
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/config.rs
  - api/apps/api-server/src/routes/plugins_and_models/plugins.rs
  - api/apps/api-server/src/routes/network_center
---

# 插件上传限制使用全局配置

## 规则

- 所有插件包上传入口使用同一个 `API_PLUGIN_UPLOAD_MAX_BYTES` 配置；不可为网络中心、模型提供者或扩展中心各自定义独立上限。
- 未配置时保持 8 MiB 默认值，配置以字节为单位且必须为正整数。

## 原因

同类插件上传具有相同的服务端内存与 multipart 风险边界。页面私有上限会造成部署行为不一致，并迫使运维为同一风险重复配置。

## 适用场景

通用插件、模型提供者、扩展中心与网络中心的包上传路由及其环境文件。
