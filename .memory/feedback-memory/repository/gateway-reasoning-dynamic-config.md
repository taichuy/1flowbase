---
title: "Gateway 推理强度应尊重动态模型声明"
memory_type: feedback
feedback_category: repository
created_at: "2026-09-19 19"
updated_at: "2026-09-19 19"
decision_policy: direct_reference
status: active
---

- 规则：分析 Gateway reasoning.effort 扩展时，先核对开始节点 model_list 中 reasoning.supported_efforts/default_effort 的动态声明；不能用入口硬编码枚举覆盖它，也不能忽略应用声明而把全部校验交给供应商。
- 原因：用户指出 UI 已允许动态配置 max 及其他强度；当前协议入口白名单与动态声明冲突。前次开发提示词只强调供应商能力校验，遗漏了应用自身的声明约束。
- 适用场景：Responses/Chat 映射、模型能力展示、应用发布快照与 AI Native 请求准入。字段格式校验、应用允许集合与上游实际能力是不同职责；运行态是否消费配置需据当前代码验证。

- 2026-09-19 用户明确缺省契约：未传 effort 采用已发布 `default_effort`；`supported_efforts=[]` 拒绝显式 effort。不能沿用“缺省不注入、空列表无限制”的旧假设。显式关闭 reasoning 的既有契约继续保留；当前阶段状态仍以 Root #2085 为准。
