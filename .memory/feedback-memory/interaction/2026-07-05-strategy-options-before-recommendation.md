---
memory_type: feedback
feedback_category: interaction
topic: strategy-options-before-recommendation
summary: 策略或方案输出必须先完整罗列可行方向，再单独给最终建议；不能先亮推荐方案再补其他方向。
keywords:
  - problem-framing
  - strategy
  - options
  - recommendation
  - 三方案
match_when:
  - 使用 problem-framing 输出方案
  - 用户要求策略建议、方向选择或设计对齐
  - 任务涉及保守 / 平衡 / 激进三方案
created_at: 2026-07-05 17
updated_at: 2026-07-05 17
last_verified_at: 2026-07-05 17
decision_policy: direct_reference
scope:
  - .agents/skills/problem-framing/SKILL.md
  - discussion
---

# Strategy Options Before Recommendation

## 时间

`2026-07-05 17`

## 规则

策略建议、方案选择或 `problem-framing` 三方案输出时，必须先完整罗列所有可行方向，再单独给最终建议。不得先写推荐方案，也不得只完整展开推荐方向而用一句话带过其他方向。

## 原因

用户指出此前回复先给了推荐方案，其他两个方案没有完整展开，违反了 `problem-framing` 对三方向决策的要求。方案对比必须先提供可比较对象，推荐才有判断基础。

## 适用场景

- 输出保守 / 平衡 / 激进三方案。
- 做 architecture / contract / permission / API / frontend-backend 设计取舍。
- 用户要求“出方案”“给策略”“重新优化 problem-framing”。
