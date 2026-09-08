---
title: AI 网关固定模板与保存即发布
date: 2026-09-08 17
decision_policy: verify_before_decision
---
用户批准并由 Codex 通过 MCP 配置：ai_gateway 记录应用；node_config 记录应用独立 LLM 配置；node_template 记录可复用 Agent 模板。模板复制后独立，目的为降低 GUI 操作心智。固定开始、IF 分发、LLM、变量聚合、回复结构，无任意工作流编辑需求。保存按钮直接保存并发布，发布成功才提示成功。无截止日期。
页面 01a073f3-c5e9-7623-9db4-1287b28366ea，Block 01a0746e-53d6-7c01-9ddd-fcf0dffb5c5e。MCP 分组 /frontstage/ai-gateway；生成接口 POST /api/ex/ai-gateway/build，输入 payload JSON 字符串。生成 Workflow 应用 01a0801e-fd6d-76c1-af0a-9b7fd524df04。
用户授权低代码和 Workflow 源码配置；产品仓库源码若必须修改，应暂停说明问题。此次没有修改产品源码。真实浏览器首次和再次保存发布通过；五项定向测试通过，未执行真实 LLM 推理。验收记录与应用已清理。后续先核对实际 MCP 配置。

2026-09-08 17 修正已验证的提示词契约问题：编辑器仅展示 templated_text，且仅保留一个 SYSTEM，后续 SYSTEM 会作为 USER。生成器改为单个 SYSTEM（配置提示词 + 请求 system）和一个 USER（请求 query）；已重新发布生成器及用户指定 test02 应用 01a0804f-0cc7-7ba2-86d2-67f3b2faa759。真实编辑器 normalization 回归测试及浏览器显示核对通过。

2026-09-08 18 Block 调用的生成 Workflow 已将固定聚合输出改为 group1，回复引用 node-aggregate.group1，与 GUI 默认变量目录对齐。test02 同步修复并发布。产品源码未修改；7 项定向测试通过。

2026-09-08 19 用户明确更正提示词顺序：SYSTEM 内先 {{node-start.system}}，空行后追加内置提示词。此决定覆盖此前内置在前的顺序。生成 Workflow 和 test02 已同步更新发布，7 项测试通过。不据此宣称缓存命中改善。

2026-09-08 21 自动生成 CRUD 删除响应契约修复：runtime_data_model_docs.rs 的 200 Deleted 原来没有 content，补齐 application/json 与 deleted:boolean schema；不改 Handler、权限、事务或分发器。生成契约和真实 Frontstage callable 删除/CSRF/持久效果两项定向测试通过。已重启本地后端，浏览器成功依次删除临时节点配置和网关记录，MCP 查询均为空。用户原有 test02 缺少 node_config 的先前部分删除状态未擅自恢复或删除。
