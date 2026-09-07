---
title: Drawer 按 Block 更新隔离方向已批准
created_at: 2026-09-08 00
updated_at: 2026-09-08 00
decision_policy: verify_before_decision
status: active
---

用户确认由当前 Root 按平衡方向实现，线上 Single Issue https://github.com/taichuy/1flowbase/issues/2005 。目标是减少 Drawer 交互时无关页面树工作，先做增量订阅，再以测量决定调度调整；保留既有状态机、generation、Worker 与视口迟滞，不修改用户 Block 代码、文案或全局样式。动机是以更小机制处理可证实热点，避免提前引入自适应调度架构。无指定截止日期。

当前按 Block 隔离已实现并定向验证，完整性能目标尚未达成：重复开关中位数未改善，释放尾部有波动；顶栏 Tooltip 联动为残余诊断入口。继续时先读 Issue 的最新验收证据，不得把本阶段描述为 Drawer 延迟全部解决。Issue 保持开放，用户验收后才关闭。
