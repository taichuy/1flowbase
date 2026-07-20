---
memory_type: feedback
feedback_category: interaction
topic: 已批准 contract 内的测试 fixture 修正不重复索取产品授权
summary: 测试应服务当前已批准目标；旧断言、旧测试和无效 fixture 直接更新或删除。只让 fixture 符合当前 contract 的机械修正不改变产品决策，不应因 QA 流程停止规则重复要求用户授权。
keywords:
  - test fixture
  - legacy test
  - reauthorization
  - QA stop rule
  - current contract
match_when:
  - 新架构重构后旧断言或无效 fixture 阻塞测试
  - QA 重复失败但修复只涉及测试前置数据
  - 判断是否需要用户再次批准修复方向
created_at: 2026-07-20 23
updated_at: 2026-07-20 23
last_verified_at: 2026-07-20 23
decision_policy: direct_reference
scope:
  - test-driven-development
  - qa-evaluation
  - problem-framing long-running work
---

# 当前 Contract 的 Fixture 修正无需重复授权

## 规则

- 测试只验证当前已批准的产品目标和 contract。
- 与当前目标冲突的旧断言、旧测试或无效 fixture 应直接更新或删除，不能要求生产代码增加兼容层来迎合。
- 只补齐当前 contract 已要求的测试前置状态、字段或 fixture 数据，且不改变产品代码、权限、数据影响、source of truth 或验收语义时，属于机械证据修复，不需要再次向用户索取产品授权。
- 长计划的停止规则用于语义、范围和风险变化；不能机械套到已经批准边界内的 fixture 修正。

## 原因

用户明确纠正：重构目标已经确认，测试本应按新目标构造。反复请求批准会把内部流程成本转嫁给用户，也会误导为存在旧实现兼容需求。

## 适用场景

重构后的 route integration、contract fixture、浏览器 seed、测试数据构造，以及旧测试期望清理。
