---
memory_type: project
topic: 应用治理改造五个 issue 已拍板并挂上 GitHub
summary: 用户确认 /settings/applications 治理改造方向；2026-07-31 已拍板删除全部 CSV 导出，把通用“导出”统一为按目标应用生成后端压缩包，AgentFlow 与 Workflow 都需支持。
keywords:
  - application
  - settings
  - lifecycle
  - publication
  - revert-to-draft
  - issue
match_when:
  - 继续实现或验收 #1286-#1290 任一 issue
  - 讨论应用发布、下线、复制、模板导出或设置页改版
  - 需要回忆用户对生命周期语义的拍板结论
created_at: 2026-07-15 15
updated_at: 2026-07-31 23
last_verified_at: 2026-07-31 23
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/application_public_api
  - web/app/src/features/settings/components/application-management
  - web/app/src/features/applications
---

# 应用治理改造五个 issue 已拍板并挂上 GitHub

## 时间

`2026-07-15 15`

## 谁在做什么

- 用户对 `/settings/applications` 提出治理改造需求，经 problem-framing 三方向对齐后拍板平衡方向，并要求先用 `gh` 挂 issue 再开发。
- AI 创建了五个 issue：
  - #1286 [待实现]应用发布生命周期闭环：已发布可退回草稿，并按触发器统一启停语义（地基，grade:g3）
  - #1287 [待实现]修复应用复制不包含流程定义的假复制缺陷（缺陷）
  - #1288 [待实现]workflow 类型应用支持模板导出导入闭环
  - #1289 [待实现]/settings/applications 改版为表格加详情抽屉，编辑表单与新增对齐（依赖 #1286）
  - #1290 [讨论]应用日志与监控页信息架构重构（后置讨论，不动代码）

## 用户拍板结论（决策原文语义）

- 编辑入口必须覆盖新增时形成的全部应用配置，但可变性按领域约束区分：名称、描述、图标、标签和 schedule 配置可编辑；`application_type` 以及 extension 的 URL/subpath、HTTP method、同步/异步模式在创建后只读。
- extension 不可变约束必须由后端写入口校验，不能只靠前端禁用控件；任何通过通用 mapping PUT 或其他绕行入口修改上述冻结字段的请求都必须被拒绝，避免注册表、mapping、发布快照和动态 OpenAPI 产生矛盾或冲突。
- 生命周期收敛为「草稿 ⇄ 已发布」双态，新增"从已发布退回草稿"转换；不引入第三个用户可见状态。
- 按触发器落地停用语义：extension = 接口取消注册；schedule = 停止调度；agent_flow 公开 API = 停止对外调用。统一为一个发布开关心智，`api_enabled` 从用户心智退位。
- 复制必须包含流程定义（真复制），按缺陷处理；环境变量默认随复制。
- 日志报表问题独立后置为讨论 issue。

## 为什么这样做

- 发布单向是后端能力缺口：port 层只有 `create_active_application_publication_version` 与 `set_application_api_enabled`，没有下线命令；workflow 类型前端被过滤掉 API 分区，连开关都不可达。
- schedule 调度只读 `workflow_schedule_triggers.enabled` 与 publication 无关，状态列会失真——实现 #1286 时必须核实调度执行的是草稿还是发布快照，再定发布语义方案（发布=创建 publication+启用 trigger，或 调度=enabled AND published）。

## #1286 实现进展（2026-07-16 核实）

- 已核实：schedule 调度真值其实**同时**依赖 publication——`workflow_schedule.rs` 的 dispatch 要求存在 active + api_enabled publication，trigger.enabled 只是附加条件。故发布语义采用「调度 = enabled AND published」，unpublish 不触碰 trigger 配置。
- 已合入 dev（commit 0ae048298）：control-plane `unpublish` 命令 + port `deactivate_application_publication_versions` + postgres/in-memory 双实现 + `DELETE /api/console/applications/{id}/api-publication` 路由 + 设置页「退回草稿」菜单项。附带修复 postgres `load_active_application_publication` 漏 `active` 过滤的存量缺陷。
- AC-001/002/003/004/006 已由 control-plane 6 测 + 路由 2 测结算；AC-005（单一开关心智）未完成。
- 剩余两项都是前端 UI，按用户偏好（前端视觉先看效果再提交）停在此等确认：① workflow 编辑器可达的发布/退回入口（workflow 应用当前 UI 无法发布，`saveWorkflowScheduleTrigger`/`publishApplicationApiVersion` 在 workflow 前端零调用方）；② `ApplicationApiPage` 把 `api_enabled` Switch 收敛为发布状态语义。

## 截止与状态

- 无硬截止；2026-07-18 用户确认并启动 #1289。已完成 extension 创建后冻结字段在 mapping 保存与 publish 入口的后端校验，以及 Settings 详情 Drawer、schedule 编辑、extension 只读摘要、成员选择器、行内发布开关和筛选结果 CSV 导出；定向测试、TypeScript、Rust static、style-boundary 与 i18n error 门禁通过。按前端视觉确认偏好，当前不提交、不推送，等待用户查看页面效果。#1287/#1288/#1290 尚未动工。

## 2026-07-31 导出语义拍板

- 用户指出 `/settings/applications` 的通用“导出”不应表达列表 CSV，而应把当前目标应用按各自模板导出为压缩包；随后明确拍板“CSV 没有用”，全部删除，不作为二级治理能力保留。
- 当前代码确认：工具栏“导出选中 / 导出筛选结果”都调用前端 CSV builder；每行 `… -> 导出模板` 是另一套能力，只支持 `agent_flow`，下载 `.1flowbase-template.json`；后端 `export_agent_flow_template` 会拒绝 `workflow`。
- 根因判断：表格治理数据导出与应用可移植制品导出复用了同一“导出”心智，但没有共享领域 owner。已确认方向是：工具栏只导出勾选应用的后端 ZIP；单行入口导出该行 ZIP；删除 CSV builder、入口、文案与测试；AgentFlow 与 Workflow 由后端各自 exporter 装配统一 archive contract。
