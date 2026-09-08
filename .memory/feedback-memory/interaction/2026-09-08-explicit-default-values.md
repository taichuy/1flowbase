---
memory_type: feedback
feedback_category: interaction
topic: 显式默认值不能替换为候选数量推断
summary: 用户指定默认代码区块时，后端应提供固定默认值，不能改成只有一个候选才自动选择。
keywords: [默认值, 代码模板, 代码区块, 后端真值]
match_when: [用户要求后端默认某个明确类型]
created_at: 2026-09-08 23
updated_at: 2026-09-08 23
last_verified_at: 2026-09-08 23
decision_policy: direct_reference
scope: [默认值需求解释]
---

## 规则

用户明确指定默认类型时，实现该固定默认值，不把它重新解释为候选数量、排序或自动发现规则。前端消费后端给出的默认值。

## 原因

本次用户纠正：要求默认“代码区块”，不是“省略类型后根据当前目录数量选唯一值”。后者会在目录扩展后改变行为。

## 适用场景

有明确默认对象、类型或模式的接口和表单需求。
