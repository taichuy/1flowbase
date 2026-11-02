---
memory_type: feedback
feedback_category: interaction
topic: codex-context-accounting-acceptance
summary: 上下文计数修复必须覆盖真实压缩判断与跨轮恢复，不能只看界面百分比或单次 usage。
created_at: 2026-11-02 00
updated_at: 2026-11-02 08
decision_policy: direct_reference
keywords: [Codex, context, usage, compaction, reasoning, resume]
---

规则：诊断上下文显示与提前压缩时，核对同一次调用的 provider usage、界面计数、实际压缩 guard 和触发原因；区分累计计费量与当前上下文，避免重复计入缓存或历史推理子集。交付验证覆盖连续新轮次、恢复会话，以及真正达到阈值仍会触发的反例。

原因：用户纠正了仅修 usage 后就认为问题结束的判断；界面正常仍可能在跨轮信号重置后提前压缩。一次请求或补齐响应头不能单独证明整个生命周期已修复。

适用场景：Codex 自定义供应商、网关 usage 投影和自动压缩计数故障。以真实运行记录确认责任层，不通过抬高窗口、关掉压缩或改写旧记录掩盖症状。


补充规则：自定义模型目录仅用于容量和能力发现时，明确隔离指令与工具执行策略，不能把协议里的占位提示词当作 Codex 默认指令。目录失败分别验证已有成功缓存和无缓存启动；容量回退与已上报 usage 分开结算。reasoning 已计入的声明来自已验证供应商合同，不能由 HTTP 成功或 WebSocket 升级推出所有模型都满足该语义。

补充原因与场景：用户指出“只配置上下文拉取”却影响运行上下文；原生目录实际携带更多策略。适用于 Codex 目录投影、客户端 metadata overlay 与多供应商 usage 契约，避免修复一个路由后误扩为全部供应商保证。
