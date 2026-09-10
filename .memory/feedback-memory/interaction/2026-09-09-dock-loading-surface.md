---
title: 编排侧栏加载时保留完整白底
memory_type: feedback
feedback_category: interaction
created_at: 2026-09-09 18
updated_at: 2026-09-09 18
decision_policy: direct_reference
---
- 规则：编排器侧栏首次懒加载时，先显示铺满内容区的白底面板，在其内部展示统一加载动画。
- 原因：用户指出只有透明空壳或悬在画布上的动画不够，需要加载阶段也有明确的面板范围。
- 适用场景：本项目编排器的节点详情、预览等局部侧栏；不因此修改全局加载组件背景。
