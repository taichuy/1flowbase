---
memory_type: feedback
feedback_category: interaction
topic: Workflow 扩展接口使用平台统一 API 授权而非应用级访问策略
summary: 普通 Application 的权限由既有 ACL 控制；Workflow 扩展接口不定义应用级访问策略或匿名 bypass。外部调用使用 User API Key，站内文档测试可使用当前 Session + CSRF；AgentFlow 继续使用专属 Application API Key。
keywords:
  - workflow extension
  - api authorization
  - user api key
  - application api key
  - access policy
  - openapi
match_when:
  - 设计或实现 Workflow /api/ex/* 认证授权
  - 在 Application 创建或编辑页面出现访问策略
  - 区分平台 API 凭证与 AgentFlow Application API Key
created_at: 2026-07-21 10
updated_at: 2026-07-22 07
last_verified_at: 2026-07-22 07
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/application_public_api
  - api/crates/control-plane/src/application_public_api
  - web/app/src/features/applications
---

# Workflow 扩展接口使用平台统一 API 授权

## 规则

Application 的业务访问权限继续由既有 ACL / permission contract 决定，API Key 只承担调用者认证，不成为应用级权限策略。Workflow HTTP Extension 不保存、不展示 `user_api_key/public` 二选一访问策略，也不开放匿名 bypass。

Workflow `/api/ex/*` 接受两组认证：外部调用使用 User API Key；站内文档测试可使用当前登录 Session + CSRF。两种认证都按调用者已有权限进入同一 ACL、workspace 与执行链，运行主体分别记录为 `user_api_key` 与 `user`。Session ID 不得进入 Scalar 表单或 Code Snippet。

AgentFlow 保留专属、应用绑定的 Application API Key contract；不把当前 Session 扩展到需要由 Key 定位应用的 AgentFlow 共享 path。

## 原因

把认证方式再次存入每个 Workflow 会复制平台统一 API 授权真值，并把“凭证是谁”错误提升为“应用权限是什么”。这会产生重复配置、公开访问旁路和前后端分支。

## 适用场景

适用于 Workflow `/api/ex/*`、dynamic OpenAPI、MCP Interface Catalog / Tool invocation，以及 Application 创建、编辑和发布 UI。不得仅隐藏前端字段而保留后端 access policy 或 public principal 分支。
