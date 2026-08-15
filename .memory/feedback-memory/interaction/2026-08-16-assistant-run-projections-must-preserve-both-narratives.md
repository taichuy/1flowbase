---
memory_type: feedback
feedback_category: interaction
topic: 助手运行过程必须同时保留有序行为叙事与完整节点叙事
summary: 分离助手运行信息时，主聊天按事件顺序展示思考、工具和阶段输出，侧栏保留全部工作流节点卡片且不显示思考；不得把主区缩成最终回答或把侧栏缩成单节点摘要。
keywords:
  - embedded assistant
  - ordered activity
  - workflow node cards
  - reasoning
  - sidebar
match_when:
  - 设计或修改内置助手的消息内容与运行过程侧栏
  - 将工作流节点、思考、工具调用或输出在主区和侧栏之间重新分配
created_at: 2026-08-16 07
updated_at: 2026-08-16 07
last_verified_at: 2026-08-16 07
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/embedded-assistant
  - web/app/src/features/agent-flow/components/debug-console
---

# 助手运行过程的双投影边界

## 规则

- 主聊天区按后端事件顺序呈现完整助手行为：思考、工具调用/结果、阶段性对话输出，以及后续重复阶段。
- 运行过程侧栏复用原完整工作流节点卡片，展示所有节点、状态和可展开详情，但不在节点卡片里重复思考文本。
- 单个当前或最后节点只能作为定位辅助，不能替代完整节点序列；只显示最终回答也不能替代主聊天的有序行为叙事。

## 原因

用户需要同时阅读助手行为发生的真实顺序和工作流节点执行情况。把两者分别压缩为最终回答与单节点摘要，会同时丢失过程连续性和节点可观测性。

## 适用场景

内置助手实时运行、历史回放、运行过程入口、节点卡片投影和 answer presentation 渲染。
