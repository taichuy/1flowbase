---
memory_type: feedback
feedback_category: repository
topic: native-default-login
summary: 原生便携版首次登录沿用 docker/.env.example 默认值，本次新功能不做旧密码迁移或重置兼容。
keywords: [native, portable, bootstrap, root, default-password]
match_when:
  - 调整原生便携版的首次登录配置和初始化行为
created_at: 2026-10-29 00
updated_at: 2026-10-29 00
last_verified_at: 2026-10-29 00
decision_policy: direct_reference
scope: [native-distribution, bootstrap-login]
---

## 规则

原生便携版首次账号密码采用构建时 docker/.env.example 的默认值，用户一键启动后直接登录。具体值以当前配置文件为准，不另外维护随机 root 初始密码。

本轮用户明确会删除旧目录再启动，要求按新功能处理，不增加旧密码迁移、重置工具或兼容流程；这不是重启时覆盖用户已改密码的授权。

## 原因

用户指出原生版登录默认值应与 Docker 配置一致，并明确否定为此次新功能额外设计旧密码重置流程。

## 适用场景

原生包初始账号密码、启动文档和本次验收；不将此范围扩张为其他持久化数据迁移政策。
