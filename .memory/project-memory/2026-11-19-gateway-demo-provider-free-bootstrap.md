---
title: "Gateway Demo 默认工作台不依赖大语言模型供应商"
memory_type: project
created_at: "2026-11-19 22"
updated_at: "2026-11-20 00"
decision_policy: verify_before_decision
status: active
tags: [gateway-demo, application-template, docker]
---

用户在 2026-11-19 确认按平衡方案修复：Docker 默认导入网关管理工作台，不要求预先配置大语言模型供应商；示例对话 AgentFlow 独立按需安装，工作台实际依赖的报表和应用 API Key 工作流保留。模板导入与应用发布继续使用现有校验，不能以关闭校验达成安装。日报 UI 需同 ID 的工作流，可先导入草稿及关闭的定时配置；实际执行在用户配置模型并发布后开启。

2026-11-20 用户进一步授权把已验收的模板推送打包，并增加默认初始化的硬门禁。门禁须在新建数据库、无大语言模型供应商、不开启额外启动开关的情况下检查候选镜像实际导入成功；服务健康不能替代安装凭据。授权覆盖发布签名模板、构建和发布通过门禁的 API 镜像，不覆盖替换或重启线上服务器实例。发布状态以签名 release、Actions run 和实际镜像 digest 为准。
