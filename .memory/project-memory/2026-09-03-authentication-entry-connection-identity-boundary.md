---
memory_type: project
topic: 登录入口、认证连接、外部身份绑定与会话内核正式分层
summary: 用户确认认证架构按 Presentation、Authentication、Identity、Authorization、Session 五层拆分；Login Entry 多对一连接，外部身份显式绑定，Session 仅由 Kernel 在 verified + bound 后签发。
keywords:
  - authentication
  - login-entry
  - authentication-connection
  - verified-identity
  - identity-binding
  - auth-kernel
  - session
match_when:
  - 扩展 root 或其他用户的登录方式
  - 新增 OIDC OAuth SAML 等外部认证 Provider
  - 修改登录入口、身份绑定或 session 签发
  - 继续 Root Issue 1982
created_at: 2026-09-03 16
updated_at: 2026-09-03 16
last_verified_at: 2026-09-03 16
decision_policy: verify_before_decision
scope:
  - api/crates/domain/src/auth
  - api/crates/control-plane/src/auth
  - api/crates/control-plane-contracts/src/ports/auth.rs
  - api/crates/storage/durable/postgres/src/auth_repository
  - web/app/src/features/auth
  - web/app/src/features/settings
  - github:issue:1982
---

# 认证入口与身份边界

## 时间

`2026-09-03 16`

## 谁在做什么

Root #1982 将原先混合展示入口、Provider 配置和 identity namespace 的 Authenticator 拆为登录入口、认证连接、已验证身份、内部用户授权和 Session Kernel 五层。认证候选已完成实现并通过集中 QA；大型 Settings 综合 fixture 活性问题由 #1986 独立治理。

## 为什么这样做

新增或复制登录入口只是 Presentation 配置，不应创建新的账号密码身份空间。外部 Provider 也不应直接选择内部用户；它只能证明外部 subject，随后由显式 binding graph 解析内部用户。

## 正式不变量

- `LoginEntry → AuthenticationConnection` 多对一。
- `VerifiedIdentity → InternalUser` 最多一对一。
- Login Entry CRUD 不改变 identity 集合。
- Session 只由 Auth Kernel 在 `verified + bound` 后签发。
- 未绑定外部身份 fail closed，不按 email 自动合并。
- `password-local` 由 Kernel 私有 credential verifier 处理；外部 Provider 的返回 contract 只能是 `VerifiedExternalIdentity`。
- `VerifiedExternalIdentity` 是不可反序列化的进程内 capability，只能经受校验的构造入口产生。

## 截止日期与后续入口

本阶段于 `2026-09-03 16` 完成候选 QA。未来扩展 OIDC/OAuth/SAML 时复用 `AuthenticationConnection + VerifiedExternalIdentity + explicit binding`，不得重新把 Provider、Login Entry 或 email 当作内部用户真值。

## 决策动机

让 UI 表单扩展、凭据验证、身份归并、权限和会话签发各自只有一个 owner；新登录方式增加 adapter，不改变内部用户和 Session Kernel 的安全不变量。
