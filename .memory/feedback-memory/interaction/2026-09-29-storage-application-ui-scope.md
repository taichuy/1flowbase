---
title: 存储优化UI验收限定应用工作流日志
date: 2026-09-29 07
feedback_category: scope_correction
decision_policy: direct_reference
---

规则：本次存储优化中，用户说“UI正常”指应用相关的编排/工作流、日志、节点详情与协议轨迹等影响面，不包含demo组件页面。不得将无关demo或平台全量体检自动作为本次交付blocker，也不据此反复请求扩大范围。

原因：用户明确纠正：“UI是指应用相关工作流日志这些，不是这些demo页面。”先前Root把范围扩大到demo，引入无关阻断。

适用场景：当前存储优化的功能保护、回归验收、续工和dev交付；其他任务按用户当次目标重新定位，不把此条外推为忽略应用范围真实故障。
