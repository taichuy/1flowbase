---
memory_type: feedback
feedback_category: interaction
topic: 加载壳不能掩盖运行时预算边界错误
summary: 区块局部加载壳用于提供友好的等待体验，但必须配合真实懒加载和分阶段运行预算；Worker 冷启动、源码获取等平台成本不得挤占用户代码执行时间。
keywords:
  - frontstage
  - loading shell
  - lazy loading
  - worker startup
  - runtime timeout
match_when:
  - 设计或实现代码区块加载、Worker 生命周期、运行超时与重试体验
  - 判断加载壳是否已经构成真实懒加载
created_at: 2026-07-20 10
updated_at: 2026-07-20 10
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - web/packages/page-runtime
  - web/packages/block-renderer
---

# 加载壳不能掩盖运行时预算边界错误

## 规则

区块局部加载壳只负责诚实表达等待状态，不能把“界面更友好”等同于“已经懒加载”。源码获取、排队、Worker 冷启动、代码准备、用户代码执行、外部 effect 与 UI 提交应有可观察的阶段边界；平台启动与调度成本不得计入用户代码执行预算。

## 原因

用户明确指出：简单 Demo 都经常 `runtime_timeout` 时，复杂代码只会更不稳定；加载中状态的产品目的，是让用户在真实加载期间获得稳定、友好的局部反馈，而不是延迟暴露同一个错误。

## 适用场景

- Frontstage 代码区块与 Schema UI 的局部加载。
- Browser Worker 的启动、复用、调度和硬终止。
- 运行超时、重试、错误展示与可观测性设计。
