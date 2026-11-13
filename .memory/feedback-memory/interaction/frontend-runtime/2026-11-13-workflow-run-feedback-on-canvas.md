---
title: "工作流试运行反馈直接呈现在画布"
memory_type: feedback
feedback_category: interaction
created_at: "2026-11-13 17"
updated_at: "2026-11-13 17"
decision_policy: direct_reference
status: active
tags: [workflow, schedule, canvas, animation, test-run]
---

- 规则：用户要求工作流与定时任务开始试运行后不显示结果抽屉；运行期间禁止画布移动，节点从开始到结束依执行进度逐个高亮。
- 状态标记：用户补充节点右上角显示小尺寸状态图标，执行中转圈、成功绿色、失败红色，尚未执行留空。
- 原因：用户希望直接观察流程如何执行，这是长期希望具备的工作流调试交互；抽屉会遮挡画布且难以对应当前节点。
- 适用场景：工作流和定时任务试运行交互的当前调整；开始节点参数表单仍沿用已确认方向。测试门禁暂停要求未被撤销。
