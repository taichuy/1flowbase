---
memory_type: feedback
feedback_category: repository
topic: native-default-login
summary: 原生正式包以 api/apps/api-server/.env.example 为配置来源，根目录提供用户可编辑的 .env，运行时读取；不再编译内嵌 Docker 默认值。
keywords: [native, portable, bootstrap, root, default-password]
match_when:
  - 调整原生便携版的首次登录配置和初始化行为
created_at: 2026-10-29 00
updated_at: 2026-10-29 07
last_verified_at: 2026-10-29 07
decision_policy: direct_reference
scope: [native-distribution, bootstrap-login]
---

## 规则

用户最新纠正：原生正式包的配置来源应为 api/apps/api-server/.env.example；发布包根目录提供可编辑的 .env，启动时读取用户文件。Docker 示例不再是原生配置真值，编译内嵌默认值不能替代可编辑的运行配置。原生所需端口、路径与环境差异需要明确处理，不能让旧 settings.json 覆盖用户配置。

本轮用户明确会删除旧目录再启动，要求按新功能处理，不增加旧密码迁移、重置工具或兼容流程；这不是重启时覆盖用户已改密码的授权。

## 原因

用户先要求统一默认登录值，随后明确纠正为使用后端配置模板并随包提供运行时 .env。用户仍不要求为试验期账号补旧密码重置流程；新增的正式版本升级需求需另行对齐保留配置和数据的方案。

## 适用场景

原生包初始账号密码、启动文档和本次验收；不将此范围扩张为其他持久化数据迁移政策。
