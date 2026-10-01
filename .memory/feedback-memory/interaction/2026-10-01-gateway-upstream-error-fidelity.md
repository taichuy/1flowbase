---
memory_type: feedback
feedback_category: interaction
topic: AI Gateway 上游错误如实返回
summary: 用户明确要求上游报错优先如实透传，AI Native 负责忠实翻译；通用脱敏与内部诊断不能替代主错误，网关自身真实异常才单独封装。
created_at: 2026-10-01 21
updated_at: 2026-10-01 21
decision_policy: direct_reference
scope:
  - AI Gateway error propagation
  - AI Native protocol translation
---

# Preserve upstream errors through the gateway

## 规则

上游已返回错误时，保持原始错误结构、字段值和消息，AI Native 与协议层仅做目标协议所需的忠实翻译；未知错误码也保留，不通过通用脱敏、内部分类或诊断文案覆盖主错误。网关自身产生的异常另行封装，并明确来源。当前用户要求重新给方案，尚未授权实施该方案。

## 原因

用户指出现有错误封装过度，要求首先保证网关透传和客户端可识别的真实错误；本轮不把通用错误脱敏作为方案目标。

## 适用场景

设计或修复供应商插件、AI Native、公开协议出口之间的错误传播，以及错误恢复和诊断附加信息。
