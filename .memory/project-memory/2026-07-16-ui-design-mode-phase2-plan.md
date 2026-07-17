---
memory_type: project
topic: UI 设计模式三级配置与真 JSX 区块编辑（#1297 平衡方案）
summary: 用户拍板 #1297 总纲的平衡方案并要求直接开工；4 个子任务按依赖实施：#1298 capability 桥接（先行地基）→ #1299 JSX 编译（sucrase+facade h()）与 #1300 三级设计态交互（可并行）→ #1301 Monaco 前言与类型注入（依赖前两者）。
keywords:
  - frontstage
  - data-capability
  - jsx-compile
  - sucrase
  - monaco
  - design-mode
match_when:
  - 实施或验收 issue 1297/1298/1299/1300/1301
  - 修改 frontstage capability dispatch、data_capabilities 路由
  - 引入 JSX 编译或修改 js-block-source-transform
  - 改造 Tab/页面/区块设计态交互
created_at: 2026-07-16 00
updated_at: 2026-07-17 18
last_verified_at: 2026-07-17 18
decision_policy: verify_before_decision
source_issue: "#1297"
scope:
  - api/apps/api-server/src/routes/frontstage
  - web/packages/page-runtime
  - web/packages/antd-facade
  - web/app/src/features/frontstage
---

# UI 设计模式三级配置与真 JSX 区块编辑

## 时间
`2026-07-16 00`

## 谁在做什么、为什么
用户确认 #1297 平衡方案（不采用保守的纯 facade 方案，不采用激进的 native-trusted 全开方案），要求 AI 直接开工实施 4 个子 issue。动机：最大化 AI 友好——让 AI 打开区块编辑器就能看到可用变量、绑定接口和组件目录。

## 关键决策
1. #1298 是地基：数据模型 CRUD 桥接为 `frontstage.data_model.record.{list,get,create,update,delete}` 5 个静态 capability，model code 走 params，不做动态 per-model 注册（kernel 每请求构建，静态 ID + 参数化 model 最简）。
2. 新增 `GET /frontstage/:workspace_id/data-capabilities` 目录 API，返回 queries/actions 描述符（含 params_schema/result_schema JSON Schema）+ 已发布模型及字段，供绑定 UI 与 Monaco 消费（同源 Catalog 原则）。
3. 每次 dispatch 在后端重查 tab 可见性（get_page_detail）+ runtime scope grant，写操作复用 runtime_engine ACL；沙箱边界不变。
4. JSX 编译选 sucrase（纯文本转换，Worker 内可跑），jsx pragma 指向 antd-facade 新增 h() 工厂，产出现有 UI Schema，插在 import 白名单 transform 之前。

## 进度（2026-07-17）
- #1298 后端已实现并通过 5 个新集成测试（frontstage_data_capability_routes.rs）；api-client 已加 listFrontstageDataCapabilities。
- #1299 已接入 sucrase + facade `h()` 真 JSX 管线，并将内置模板转为 JSX；当前默认模板进一步收敛为无数据调用、无副作用的最小示例。
- #1300 三级选择边界与晶莹蓝基线已实现。用户于 2026-07-17 18 进一步确认紧凑修正：页面标题单行约 44px，移除伪区块空画布和常驻同步状态，“创建区块”直接生成官方默认 JSX 区块，成功后选中但不自动打开编辑器，区块工具条直接显示配置与 JSX 编辑图标。该修正已在当前工作树实现，待用户视觉验收，尚未提交 / push。
- #1301 尚未实现，继续作为 Monaco 前言与类型注入的独立任务。
