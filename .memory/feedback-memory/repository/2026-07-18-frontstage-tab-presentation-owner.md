---
memory_type: feedback
feedback_category: repository
topic: Frontstage Tab 展示必须是页面持久化配置而非设计模式副作用
summary: 用户指出当前只有开启设计模式才显露默认 Tab 的心智模型有问题；后续讨论或实现 Frontstage 页面层级时，应将 Tab 展示归为页面持久化配置，并和设计模式中的编辑控件分离。
keywords:
  - frontstage
  - page tabs
  - persisted configuration
  - design mode
  - hierarchy
match_when:
  - 调整 Frontstage 页面级配置或 Tab 展示逻辑
  - 设计模式的临时 UI 状态可能影响运行态结构
  - 讨论 Page、Tab 与 Block 的数据归属
created_at: 2026-07-18 20
updated_at: 2026-07-18 20
last_verified_at: 2026-07-18 20
decision_policy: verify_before_decision
scope:
  - web/app/src/features/frontstage
  - api/crates/domain/src/frontstage.rs
  - api/apps/api-server/src/routes/frontstage
---

# Frontstage Tab 展示归属

## 规则

不能以设计模式是否开启决定页面是否具有或展示 Tab 容器。页面自身应持久化其内容呈现模式；设计模式只额外提供 Tab 的创建、排序、配置等编辑控件。

## 原因

设计态是临时编辑状态，不是页面结构真值。将两者混用会让单一默认 Tab 在运行态和设计态出现不一致的结构，也阻碍 Page → Tab → Block 的清晰归属。

## 适用场景

Frontstage 的页面配置、动态路由、Tab Document、区块布局与设计态交互调整。
