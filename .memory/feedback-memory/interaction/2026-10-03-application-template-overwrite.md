---
title: "内置应用模板直接覆盖与自动更新开关"
memory_type: feedback
feedback_category: interaction
created_at: "2026-10-03 12"
updated_at: "2026-10-03 12"
decision_policy: direct_reference
status: active
tags: [application-template, gateway, docker]
---

- 规则：用户确认内置应用模板更新直接覆盖已有资源，不检测或保护用户修改；增加后端自动更新配置，false 时空库不初始化、已有库不主动覆盖。版本与内容摘要用于判断包更新，不用于用户修改保护。
- 原因：用户明确选择由部署者控制自动更新，不需要逐资源冲突与合并机制。
- 适用场景：Gateway demo 内置应用模板的 Docker 初始化及升级。此处“数据来源”明确为 `/settings/data-models` 的用户可见数据模型定义及关系，不是外部数据库连接凭据。
