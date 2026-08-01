---
memory_type: feedback
feedback_category: repository
topic: 工作流发布不得冻结供应商运行时能力
summary: 工作流发布只冻结工作流语义、供应商实例引用与路由策略；供应商当前安装版本及 capability 属于供应商运行时，应在每次调用边界解析，插件升级不得要求重新发布工作流。
keywords:
  - workflow-publication
  - provider-capability
  - compiled-plan
  - ai-gateway
  - runtime-resolution
match_when:
  - 设计或诊断工作流发布与模型供应商插件升级关系
  - 决定 capability 应进入 CompiledPlan 还是在调用时解析
  - 插件升级后旧发布应用出现 semantic_capability_unsupported
created_at: 2026-08-01 09
updated_at: 2026-08-01 09
last_verified_at: 2026-08-01 09
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime
  - api/crates/plugin-framework
  - AI Gateway
---

# 工作流发布不得冻结供应商运行时能力

## 规则

工作流发布只拥有节点图、模型选择、供应商实例引用和工作流路由策略。供应商当前安装、manifest capability、可用状态和包身份属于供应商运行时生命周期，不能复制进工作流发布快照后作为长期兼容性真值。

AI Gateway 应在每个 Provider attempt 的调用边界，把请求所需 canonical semantics 与当前已验证安装的 capability 做匹配，并保证校验与实际调用绑定到同一个安装身份。插件升级后的能力应从下一次调用生效，不得要求重新发布工作流。

## 原因

工作流发布既不能观察也不能控制后续插件升级，让它持有 provider capability 会造成跨生命周期耦合和陈旧快照。重新发布工作流只能绕过陈旧数据，不能修复 owner 错置。

## 适用场景

- AI Gateway 的 fixed route 与 failover route 选择。
- Provider 插件安装、升级、禁用或 capability 变化。
- CompiledPlan 与运行时供应商目录的职责划分。
