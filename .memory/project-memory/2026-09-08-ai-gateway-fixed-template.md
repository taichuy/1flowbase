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

2026-09-09 16 用户指定参考 Block 01a08168-4d01-7341-9c16-6ca60be3a0ae（Drawer 示例页 01a07764-7e64-7ca3-b2f4-b8df39117fa4）替换网关抽屉。参考为内嵌 ResizableDrawer 壳，不是跨 Block import。目标已通过 MCP 使用同款壳（默认保留 800，480–1200、拖拽/键盘与页面宽度记忆），保存期间不可关闭。浏览器键盘调宽、重开记忆、Agent Modal 与移动端显示通过。仅更改低代码源码。

2026-09-09 22 用户批准 Single Issue #2019 开发画布键盘归属修复，并进一步明确批准将第三方 Modal 焦点修复纳入同一 Issue。Codex 已实现并推送 dev 提交 0f186996d，Issue 阶段为 user-acceptance；无截止日期。动机为设计模式下区块表单能够正常输入，不将宿主事件细节泄漏给 Block 作者。PageCanvas 仅消费自身激活，配套固定 dialog/util 依赖补丁解决开场动画抢焦点、Shadow DOM 焦点锁和直接卸载 Modal 恢复触发器问题；未修改 Block、MCP、接口或用户记录。42 项定向测试及真实设计/浏览模式逐字输入、换行、模板选择、Escape/重开通过。系统中文输入法选字仍待用户验收；复用当前代码与 Issue #2019，不把本次证据扩大为 Native React 全基座验收。补丁边界/升级移除说明见 web/patches/rc-dialog-shadow-focus.md。

2026-09-10 09 用户已确认开放开始节点参数、85% 压缩阈值、空模型走默认 Agent。新版 Block 和生成 Workflow 已经通过 MCP 保存发布，但目标应用 01a08675-0709-7a73-b4b1-161c636b74c1 尚未更新发布。多次用户重启/更新后运行验证仍阻塞：ai_gateway_build 最近失败运行 01a088f9-c2d5-78f2-8255-b0fac05a7b01 报 ephemeral_protocol_context_store_unavailable；agent_flow_debug_node 调试 node-router 后 api-server 栈溢出退出。请求证据 tmp/ai-gateway/reproduce-builder.json、reproduce-if-debug.json；日志 tmp/logs/api-server.log。用户尚未授权产品源码修复；只读排查，不再重复触发崩溃。后端修复后继续发布和浏览器验收，不重做已通过的本地测试。

2026-09-10 10 只读诊断已收敛，详见 tmp/ai-gateway/diagnosis/findings.md。生成错误由两点叠加：ProtocolContextEnvelope 的 Serde 派生接受 ["1flowbase"] 数组，扫描器误把生成文档中的 mcp_instance_ids 当协议上下文；Workflow ex.rs 服务构造缺 provider_transport_store（调试入口也缺）。MCP 栈溢出在独立 7801 GDB 进程复现：内部 HTTP dispatch 叠加大 async 帧（最大约446 KiB）及目录 schema JSON 解析；同请求直连 HTTP 不崩溃，16 MiB 线程栈 MCP 不崩溃。两种存活请求都返回 if_else 单节点预览未实现，路由须整流验证。未修改产品源码/目标配置，临时诊断 key/session 已撤销，进程已停止。仅注入存储会把误识别的 MCP ID 数组替换成 locator，因此分类器必须同时修正。

2026-09-10 10 用户要求针对本次故障及接口生命周期重构，以 problem-framing 从第一性原理分析成熟架构、数据结构和数学关系；当前任务转入架构决策，尚未授权后端实现。Codex 建议在现有统一调用内核上收敛类型识别与依赖装配、受管子调用执行边界，并由后端业务命令拥有保存发布一致性；此为待确认方向，不是已批准方案。保留原 GUI 与目标应用发布任务，待运行时问题解决后继续；无截止日期。

2026-09-10 11 用户明确批准创建 Issue 并先处理后端源码，覆盖前文尚未授权的阶段状态；同时明确前端 Block / 三表固定模板配置暂不处理。当前 Single Issue #2021 为唯一源码验收入口；Codex 已完成协议上下文分类、必需存储装配及嵌套 Future 占栈修复，默认栈 HTTP/MCP 场景通过，详见 Issue 与 tmp/test-governance/issue-2021/qa-report.md。用户应用配置未改，原 GUI 和目标应用发布仍待后续恢复；无截止日期。后续勿将源码链路通过解释为 IF 单节点预览已支持或目标应用已发布。
