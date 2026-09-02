---
memory_type: project
topic: Canonical Interface Contract 与 Invocation Lifecycle 架构方向
summary: 用户确认在 beta 基线复用 interface-runtime，以 Definition、Protocol Binding、Typed Principal Profile、Compiled Plan 和完整 Lifecycle/Receipt 收敛后端调用面；#1944 已进入 phase:ready。
keywords:
  - canonical-interface
  - invocation-lifecycle
  - typed-principal
  - interface-runtime
  - issue-1944
created_at: 2026-08-29 18
updated_at: 2026-08-29 18
last_verified_at: 2026-08-29 18
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1944
  - https://github.com/taichuy/1flowbase/issues/1893
  - /home/taichuy/git/1flowbase_latest/docs/adr/2026-08-29-backend-request-lifecycle-current-architecture.md
---

# Canonical Interface 与调用生命周期方向

## 谁在做什么

用户已确认平衡架构方向。后续开发会话以 `beta` worktree 为代码事实，按 #1944 的 IF-F01～IF-F08 Work Packet 集中装配，不拆分逐层审核 Issue，也不运行 per-Packet QA。

## 为什么这样做

当前 Console、Public Auth、Application API、MCP 与 Internal/Background 分别装配认证、授权、Handler 和协议投影；只有少量生产路径进入 `InterfaceInvocationKernel`。需要先统一调用者可观察的接口语义和生命周期，才能为插件提供稳定的空间坐标、时间身份与可还原 receipt，同时保持现有功能完整。

## 已确认决策

- 复用现有 `interface-runtime`，本阶段不新增万能 crate。
- 分离 Canonical Interface Definition、Protocol Binding 与 Compiled Invocation Plan。
- 采用 `InvocationEnvelope<Input, Principal>` 和类型化 Principal Profile；当前只批准有源码证据的 Public、User、Application。
- 不引入万能 Caller、万能 RequestContext 或原始凭证下沉。
- Domain/Application 继续拥有业务规则与 transaction invariant；Interface Kernel 不拥有 Repository 或万能事务。
- 插件只能进入显式批准的 typed point/phase；普通插件只见裁剪后的 PrincipalSummary 和 typed facts。
- Resolve-time plan 与 dispatch-time Runtime generation 分别冻结。
- Interface completion 与 Domain Event/Lifecycle Outbox delivery truth 分离。
- 全部 Packet 装配成冻结 assembly 后只运行一次 fresh centralized QA。

## 为什么要做

目标是让后端输入、输出、stream、error、terminal、身份和扩展点形成可编译、可版本化、可审计的统一 Interface 语义，而不以重写业务功能、泄漏凭证或制造万能抽象为代价。

## 截止日期与下一步

无固定截止日期。#1944 已进入 `phase:ready`；下一步由开发会话先冻结 Coverage Inventory 和集中 Test Batch，再按 Work Packet Ledger 实施。Issue 与代码事实发生变化时必须回到 beta 当前 SHA 复核。
