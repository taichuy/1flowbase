---
memory_type: feedback
feedback_category: repository
topic: 可选扩展包故障必须与基础能力隔离
summary: 可选扩展包未安装或服务不可用时，不得导致基础 catalog 或不依赖该扩展的区块失败；只有实际导入扩展依赖的区块才应收到针对该依赖的错误。
keywords:
  - optional extension
  - external npm
  - failure isolation
  - dependency catalog
  - native block
match_when:
  - 设计或修复 External npm Pack、可选扩展 catalog 与内置区块依赖装配
  - 可选服务故障导致基础功能级联失败
created_at: 2026-08-12 12
updated_at: 2026-08-12 12
last_verified_at: 2026-08-12 12
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage/api/external-npm.ts
  - web/app/src/features/frontstage/api/block-catalog.ts
  - Native React block dependency resolution
---

# 可选扩展包故障隔离

## 规则

可选扩展包缺失、未启动或暂时不可用时，基础 catalog 与内置模块必须继续工作。平台应在依赖解析时按区块的真实 import 决定影响范围：未导入扩展模块的区块不受影响；实际导入扩展模块的区块才失败，并报告扩展未安装、不可用或模块不存在的真实原因。

## 原因

可选扩展的可用性不是基础能力成立的必要条件。把两个来源放入同一个失败域，会将局部依赖故障误报为内置模块未授权并扩大影响范围。

## 适用场景

External npm Pack、插件附加模块、前端组件扩展目录以及其他可选依赖源与基础 catalog 的组合装配。
