---
memory_type: feedback
feedback_category: interaction
topic: 旧断言与无效测试不得阻断已确认的新 contract
summary: 测试必须服务当前已确认目标；旧断言直接更新，重复或不稳定且已有等价覆盖的测试直接删除，不为旧测试增加兼容代码，也不把无效测试失败误判成实现 blocker。
keywords:
  - stale test
  - invalid test
  - compatibility
  - current contract
  - qa blocker
match_when:
  - 新 contract 已确认但旧测试仍失败
  - 单测单独通过但在测试集合中因时序或污染失败
  - 需要判断测试失败是否阻断当前实现
created_at: 2026-07-20 23
updated_at: 2026-07-20 23
last_verified_at: 2026-07-20 23
decision_policy: direct_reference
scope:
  - test-driven-development
  - qa-evaluation
  - long-running work
---

# 旧断言与无效测试不得阻断已确认的新 contract

## 规则

先判断测试是否仍表达当前验收目标。旧断言更新；重复、无效或受测试基础设施污染且已有等价真实性证据的测试删除。不得为了旧测试增加 alias、fallback、兼容分支，也不得仅因无效测试反复失败就停止产品实现。

## 原因

测试是验收证据，不是产品真值。当前 contract 与可观察目标才是判断标准；让产品兼容过期测试会倒置 source of truth。

## 适用场景

适用于 contract 重构、架构演进、测试期望过期、测试重复覆盖和测试顺序污染。若删除后造成真实验收点无证据，必须补一个直接覆盖当前目标的最小有效测试。
