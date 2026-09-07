---
title: "接口管理与插件开放的阶段边界"
memory_type: feedback
feedback_category: interaction
created_at: "2026-09-07 15"
updated_at: "2026-09-07 15"
decision_policy: direct_reference
status: active
---

规则：先完成接口生命周期管理，再审计三级插件可时空组合性开放；不能为了第一阶段验收而擅自实现第二阶段manifest/loader/生产插件接线。

原因：生命周期节点及typed执行能力已有，不等于所有真实插件加载已经开放。某接口没有配置插件、扩展plan为空是正常状态；声明/实现不一致或绕过已绑定适用计划才是接口管理缺陷。用户纠正了将两阶段耦合并因此请求P6a额外批准的做法。

适用：接口完整性、Hook、Registry与插件loader衔接的规划/审计。先区分接口契约证据和生产插件开放证据；后者的缺口记录到后续阶段，不能用受控Kernel fixture冒充生产插件已验收，也不把缺少插件接线直接判成接口生命周期未完成。
