---
memory_type: feedback
feedback_category: interaction
topic: native-internal-compaction-explicit-opt-in
summary: 自动压缩仅由已发布LLM节点严格布尔true授权；助手与外部接入采用同一链路，关闭完全绕过本功能。
created_at: 2026-10-27 08
updated_at: 2026-10-27 21
decision_policy: direct_reference
keywords: [compaction, AI Native, embedded assistant, LLM node, context, 2685]
---

# Native 自动压缩的显式授权边界

规则：唯一用户开关是已发布节点 `config.context_policy.auto_compact === true`。false、缺省、null、字符串true均不启用；助手、外部来源、长输入、路由默认、客户端参数不得隐式授予权限。关闭在本功能计数/估算、checkpoint读写、摘要、预算拦截和压缩重试之前返回；原有计费、usage、上下文快照和供应商协议错误仍沿原owner处理。

开启后完整可读的外部历史也可压缩。预算使用既有实际供应商/路由窗口，含输出预留与安全余量；256K/85%仅是特定助手验收配置，不是全局上限。超过窗口先按工具闭合单元有界分批，保留原文、权限、当前用户、工具关联和动作状态；不可缩减或仅opaque/delta明确失败。summary独立Generate不执行工具、不递归、不回填原客户端native body。

原因：用户多轮纠正了旧#2685“助手默认开、外部一律禁用”的语义；关闭不应变成服务端替客户端兜底，也不能吞掉供应商超限错误。完整可见但tokenizer不建模的工具定义，与不可解释opaque不同；可完整覆盖时诚实退化为generic/provider_estimate并保留原因，不能伪称精确计数或安全上界。

适用场景：工作流LLM、内置助手、外部客户端入口的自动压缩开发与验收。最新用户规则高于旧issue/历史测试；同一最终adapter请求才可比较计数与provider usage，缺失cache字段为unknown。

补充规则：当前用户请求按canonical消息语义识别；纯user-role tool_result不能覆盖此前真实请求，工具输出内部文本不是用户指令。混合用户文本与工具回执时保护完整因果组，selection与checkpoint读取验证共用规则。原因是角色不等于内容来源；仅检查最后user会摘要掉当前请求。适用于工具续跑与旧checkpoint复用的原文保留验收。
