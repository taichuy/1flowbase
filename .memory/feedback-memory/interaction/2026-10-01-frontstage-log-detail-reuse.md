---
memory_type: feedback
feedback_category: interaction
topic: Frontstage 总日志页直接复用应用日志详情
summary: 总日志页的运行详情、对话与轨迹应在原页打开带完整样式的共享组件，不通过 URL 跳转到单应用日志页。
keywords:
  - frontstage
  - application logs
  - run detail
  - trace
match_when:
  - 设计或修改跨应用运行日志的查看入口
created_at: 2026-10-01 18
updated_at: 2026-10-01 18
last_verified_at: 2026-10-01 18
decision_policy: direct_reference
scope:
  - web/app/src/features/applications
  - web/app/src/features/frontstage
---

# Frontstage 总日志页直接复用应用日志详情

## 规则

总日志列表的“运行详情”和“对话与轨迹”直接在当前页面打开应用日志已有的详情、对话与轨迹组件，保留列表位置和筛选状态。共享组件必须连同其样式和浮层作用域一起复用，实际检查运行详情、轨迹节点及移动端呈现。

## 原因

用户指出把操作列做成跳转链接割裂了总日志页的查看流程，并明确希望抽象共享组件复用既有能力。随后指出仅有组件结构而缺少样式，不能算封装完整。

## 适用场景

设计或修改总日志页、应用日志页之间的详情入口与共享组件边界时。
