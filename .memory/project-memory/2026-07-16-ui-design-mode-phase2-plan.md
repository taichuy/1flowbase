---
memory_type: project
topic: UI 设计模式三级配置与真 JSX 区块编辑（#1297 平衡方案）
summary: 用户拍板 #1297 平衡方案并要求直接开工；通用 JSX 区块现已作为 Boot Core 管理的系统内置稳定容器注册，创建流程直接保存后端 Catalog 模板，未来能力继续通过受控组件模块 Catalog 扩展。
keywords:
  - frontstage
  - data-capability
  - jsx-compile
  - sucrase
  - monaco
  - design-mode
  - builtin-jsx-block
  - frontend-block-catalog
match_when:
  - 实施或验收 issue 1297/1298/1299/1300/1301
  - 修改 frontstage capability dispatch、data_capabilities 路由
  - 引入 JSX 编译或修改 js-block-source-transform
  - 改造 Tab/页面/区块设计态交互
created_at: 2026-07-16 00
updated_at: 2026-07-17 22
last_verified_at: 2026-07-17 22
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
5. 用户确认通用 JSX 区块是系统内置的基础执行容器，稳定贡献标识继续使用 `1flowbase:frontstage.js-ui-block`；`block-sdk`、`antd-facade` 与未来完成安全适配的组件属于它的受控导入模块 Catalog。组件扩展不等于新增区块类型，也不得以前端 fallback 代替后端注册。

## 进度（2026-07-17）
- #1298 后端已实现并通过 5 个新集成测试（frontstage_data_capability_routes.rs）；api-client 已加 listFrontstageDataCapabilities。
- #1299 已接入 sucrase + facade `h()` 真 JSX 管线，并将内置模板转为 JSX；当前默认模板进一步收敛为无数据调用、无副作用的最小示例。
- #1300 三级选择边界与晶莹蓝基线已实现。用户于 2026-07-17 18 进一步确认紧凑修正：页面标题单行约 44px，移除伪区块空画布和常驻同步状态，“创建区块”直接生成官方默认 JSX 区块，成功后选中但不自动打开编辑器，区块工具条直接显示配置与 JSX 编辑图标。
- 2026-07-17 22 已补齐系统 JSX 区块注册：新增 `api/plugins/capability-plugins/1flowbase` 官方内置包，稳定贡献标识为 `1flowbase:frontstage.js-ui-block`；Boot Core 幂等安装并投影 Catalog，`source_kind=builtin` 的 frontend block 对所有 workspace 直接可见，不依赖 assignment，且禁止通过插件管理删除。前端直接保存 Catalog 的 `code_template`，不再另造本地模板。
- 定向证据：API route / 幂等 bootstrap / builtin 全 workspace 可见测试通过，builtin 不可删除测试通过，前端 design-controls 13 项通过，TypeScript 通过，Rust 静态门禁 0 warning。隔离 schema 真实启动后 `/api/console/frontend-blocks` 返回官方贡献，Playwright 从顶部导航进入动态页面、创建并渲染 JSX 示例；数据库确认保存代码与 Catalog 模板相等。截图：`tmp/page-debug/builtin-jsx-validation-after/page.png`。
- 2026-07-17 23 已在标准 `dev-up` 环境复验：修复首次生成 Web `.env` 丢失 worktree 端口的问题，服务运行于 `3200/7900/7901` 并连接独立 database `1flowbase_latest`；完成旧 console policy 的 preview/apply/smoke/finalize 后，真实浏览器从顶栏进入 `JSX 演示`，点击 `Create block` 成功渲染内置示例。直接顶栏页面无侧栏时正文被共享布局丢弃的问题也已补回归并修复。截图：`tmp/page-debug/latest-dev-up-jsx-created.png`。
- 本轮新增注册修正仍未 commit / push，等待用户确认可见效果。
- #1301 尚未实现，继续作为 Monaco 前言与类型注入的独立任务。
