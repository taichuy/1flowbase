---
memory_type: feedback
feedback_category: architecture_boundary
topic: 前端性能优化必须由真实交互场景和需求图驱动
summary: 用户拒绝遇到请求爆炸后再用固定分包数量或强制阈值止痛；前端开发与生产性能方案应建模场景、模块共现、网络与主线程成本，并用真实本地和公网轨迹校准约束。
keywords:
  - frontend performance
  - scenario demand
  - bundle graph
  - chunking
  - interaction budget
created_at: 2026-08-31 00
updated_at: 2026-08-31 00
decision_policy: direct_reference
scope:
  - web/app
  - scripts/node/production-bundle-profile
---

# 前端性能优化使用场景需求模型

## 规则

- 不以“每 N 个模块一个包”、仅限制单文件大小或事后增加固定阈值作为完整性能架构。
- 先定义用户交互场景、每个场景所需模块集合、发生概率、缓存状态和时限，再依据模块共现图、网络成本、主线程成本与缓存失效成本形成确定性分区。
- 预算是可观测 SLO 和优化约束，必须由本地与公网真实浏览器轨迹验证和校准，不能替代根因治理。

## 原因

只优化总字节可能产生超大 vendor，只优化单 chunk 大小又可能产生请求爆炸；两者都没有表达“某次交互究竟需要哪些能力”。性能复杂度应由掌握构建图和运行轨迹的 Demand Planner 吸收，而不是泄漏为页面中的零散懒加载判断。
