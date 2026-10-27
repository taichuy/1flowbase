---
memory_type: feedback
feedback_category: interaction
topic: native-internal-fallback-compaction
summary: 用户将压缩需求明确为 AI Native 内部的保底机制，复用现有聊天与运行记录，覆盖内置助手及显式开启自动压缩的无客户端 LLM 节点。
created_at: 2026-10-27 08
updated_at: 2026-10-27 08
decision_policy: direct_reference
keywords: [compaction, AI Native, embedded assistant, LLM node, context]
---

# Native 内部保底压缩范围

规则：本需求由 1flowbase AI Native 在内部组织摘要生成与续跑，不以远程压缩端点或映射协议兼容修复为前置条件。优先复用现有 AI 助手执行能力及已持久化的摘要、系统提示词、用户输入、最终回答和运行过程事实。

原因：用户纠正上一轮将供应商原生压缩和公开协议差异带入主方案的范围偏移；实际目标是在有限上下文下让 1flowbase 内部任务持续工作。

适用场景：内置 AI 助手对话执行途中，以及无客户端调用、显式开启自动压缩的 LLM 节点。具体压缩策略、配置默认值与实现方案仍在讨论，本条不代表实施授权。
