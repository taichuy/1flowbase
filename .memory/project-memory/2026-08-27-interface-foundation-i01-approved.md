---
memory_type: project
topic: Interface Foundation I-01 架构获批并进入实施准备
summary: 用户于 2026-08-27 接受 #1911 的六项 typed 公共原语与串行 Delivery Map；#1912 I-01 已在 beta@fc0b4d44e 完成 QAF2 和 fresh QA，进入用户验收；I-02～I-06 继续 HOLD。
keywords:
  - issue 1893
  - issue 1911
  - issue 1912
  - interface-runtime
  - dynamic interface registry
  - interface invocation kernel
  - actor context
match_when:
  - 实现或审核 #1912 I-01
  - 继续拆分 #1893 的 I-02～I-06
  - 判断 Authentication、Interface Registry 或协议兼容 owner
created_at: 2026-08-27 00
updated_at: 2026-08-28 00
last_verified_at: 2026-08-28 00
decision_policy: verify_before_decision
status: active
scope:
  - https://github.com/taichuy/1flowbase/issues/1893
  - https://github.com/taichuy/1flowbase/issues/1911
  - https://github.com/taichuy/1flowbase/issues/1912
  - api/crates/interface-runtime
---

# Interface Foundation I-01 获批

## 谁在做什么

独立开发会话以 #1912 为唯一活动 Delivery，在 beta assembly 上先完成只读 Scout、dependency closure、I01-WP1～WP4 和 Test Batch 冻结，再串行实现并提交 assembly。Root #1893 继续作为计划与最终验收真值。

## 为什么这样做

真实插件场景证明统一 Interface Identity、Registry 与 Invocation Kernel 是首个可复用纵向基础；先落 I-01 可以验证新 `interface-runtime` 是否为被生产入口消费的深模块，再决定 Hook、Lifecycle、Worker 和 SDK，避免一次冻结过多公共 Contract。

## 硬边界

- Authentication credential 解析属于协议 Adapter/Gateway；Kernel 从已认证 `ActorContext` 开始。
- `interface-runtime` 不依赖 Axum、api-server、control-plane 实现、plugin-framework、Runtime Host 或 Storage。
- 协议兼容范围按 contract family 显式声明，不承诺统一 N/N-1。
- 外部 API、权限结果、schema、数据、Runtime、wire 和官方插件保持不变。
- 只授权 #1912；I-02～I-06 继续 HOLD。

## 当前阶段与下一事件

无日历截止日期。#1912 I-01 已冻结在 `beta@fc0b4d44e43629de700869e1fafc4ac6ba0c14c7`：unit typed request、bind-time narrow Query Port、单次 Kernel AuthZ 和在途 deadline 均有 fresh QA 与独立定向复核证据。下一事件是用户验收 #1912；Root AC 仍不提前结算，I-02～I-06 继续 HOLD。
