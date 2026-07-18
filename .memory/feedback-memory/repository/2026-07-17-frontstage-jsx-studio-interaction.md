---
memory_type: feedback
feedback_category: repository
topic: Frontstage JSX 区块设计态与编辑体验
summary: Frontstage 区块操作必须复用左侧页面树的紧凑 hover 图标语言；配置与代码编辑应进入同一个共享可拉伸 JSX Studio，并把接口、变量、组件和配置投影到编辑器上下文，不能用割裂的只读配置抽屉代替。
keywords:
  - frontstage
  - JSX Studio
  - Monaco
  - capability connector
  - resizable drawer
  - hover actions
match_when:
  - 修改 Frontstage 页面、Tab 或区块的设计态操作入口
  - 修改 JSX 区块代码编辑、配置、接口绑定或变量注入体验
created_at: 2026-07-17 23
updated_at: 2026-07-17 23
last_verified_at: 2026-07-17 23
decision_policy: direct_reference
scope:
  - web/app/src/features/frontstage
  - web/app/src/shared/schema-ui
  - api/apps/api-server/src/routes/frontstage
---

# Frontstage JSX Studio 交互规则

## 规则

- 区块 hover 操作复用左侧页面树的微型图标尺寸、间距、显隐和选中语言；仅换成相同颜色但保留白色胶囊工具条不算统一。
- 配置图标和编辑图标进入同一个共享可拉伸 JSX Studio，只改变初始焦点；不要继续维护两个割裂的固定宽度抽屉。
- 接口连接器、可用变量、组件目录和结构化配置必须在 Monaco 中形成可见前言、类型或可插入代码，并与运行时允许能力同源；只读 `Descriptions` 不是配置能力。

## 原因

用户需要统一、紧凑的设计态交互，也需要开发者和 AI 打开编辑器即可知道可用能力。割裂的配置查看器既不能完成绑定，也不能保证编辑器提示、持久化配置和运行时白名单一致。

## 适用场景

Frontstage 页面设计模式、JSX 区块工具条、Monaco 编辑器、区块配置、capability Catalog 和运行时绑定策略。
