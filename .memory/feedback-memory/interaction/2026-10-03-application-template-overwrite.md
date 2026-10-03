---
title: "内置应用模板直接覆盖与自动更新开关"
memory_type: feedback
feedback_category: interaction
created_at: "2026-10-03 12"
updated_at: "2026-10-03 14"
decision_policy: direct_reference
status: active
tags: [application-template, gateway, docker]
---

- 规则：用户确认内置应用模板更新直接覆盖已有资源，不检测或保护用户修改；增加后端自动更新配置，false 时空库不初始化、已有库不主动覆盖。版本与内容摘要用于判断包更新，不用于用户修改保护。
- 原因：用户明确选择由部署者控制自动更新，不需要逐资源冲突与合并机制。
- 适用场景：Gateway demo 内置应用模板的 Docker 初始化及升级。此处“数据来源”明确为 `/settings/data-models` 的用户可见数据模型定义及关系，不是外部数据库连接凭据。

- 规则：应用模板按类别、目录与单项资源维护，发布 ZIP；应用模板市场参考 agent-flow 的分页、搜索与正式发布链路，不能把整包大 JSON 放进列表接口。提供保存选择与目标目录的一键导出更新脚本。
- 原因：用户退回仅保存巨大 template.json 的实现，认为不可维护；仅推送源码不等于模板目录中可发现和下载安装。
- 验收边界：用户要求本地空 Docker 构建验证时，必须实际完成从源码构建及空库导入/升级；预构建底图加本地二进制只能作为局部证据，不能据此宣称整个任务完成。资源限制要调整任务自身的并发和执行次序，再继续完成已授权验证。
