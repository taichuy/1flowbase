---
title: "轨迹窗口使用共享壳与自动摘要加载"
memory_type: feedback
feedback_category: interaction
created_at: "2026-09-22 10"
updated_at: "2026-09-22 10"
decision_policy: direct_reference
status: active
tags: [trajectory, frontend, interaction]
---

- 规则：轨迹入口复用可拖拽/缩放的共享窗口壳；提供DeepSeek harness式三泳道时间范围选择，不只做逐步点击定位条。打开后自动串行渐进加载摘要，正文按需，关闭或隐藏停止继续分页。
- 原因：用户指出普通Modal无法拖拉伸、首屏后靠手动更多导致轨迹过少；需要先打开再自动逐步填充，并筛选关注的请求时间范围。
- 适用场景：客户端请求和LLM内部事件轨迹视图；保持现有节点关系与输入处理输出职责，不因时间轴更改采集架构。
