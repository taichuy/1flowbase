---
memory_type: feedback
feedback_category: interaction
topic: codex-context-accounting-acceptance
summary: 上下文计数修复必须覆盖真实压缩判断与跨轮恢复，不能只看界面百分比或单次 usage。
created_at: 2026-11-02 00
updated_at: 2026-11-02 00
decision_policy: direct_reference
keywords: [Codex, context, usage, compaction, reasoning, resume]
---

规则：诊断上下文显示与提前压缩时，核对同一次调用的 provider usage、界面计数、实际压缩 guard 和触发原因；区分累计计费量与当前上下文，避免重复计入缓存或历史推理子集。交付验证覆盖连续新轮次、恢复会话，以及真正达到阈值仍会触发的反例。

原因：用户纠正了仅修 usage 后就认为问题结束的判断；界面正常仍可能在跨轮信号重置后提前压缩。一次请求或补齐响应头不能单独证明整个生命周期已修复。

适用场景：Codex 自定义供应商、网关 usage 投影和自动压缩计数故障。以真实运行记录确认责任层，不通过抬高窗口、关掉压缩或改写旧记录掩盖症状。
