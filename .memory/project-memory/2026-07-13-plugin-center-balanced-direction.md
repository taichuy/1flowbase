---
memory_type: project
topic: 插件中心统一发现与类型化安装平衡方向
summary: 用户确认在 Settings 侧边栏首位新增插件中心，统一发现插件、MCP 配置包和工作流模板，但保留各自安装与持久化生命周期；初始化内容采用打包时锁定、首次启动幂等应用。模型供应商允许从插件中心直接安装，但现有设置页面、Registry、包 ID、接口、物理源码目录和已安装数据不得受影响。
keywords:
  - plugin-center
  - official-plugins
  - model-provider
  - backward-compatibility
  - mcp-bundle
  - workflow-template
  - bootstrap-profile
match_when:
  - 设计或实现 Settings 插件中心
  - 统一官方插件、MCP 配置包与工作流模板目录
  - 调整模型供应商仓库目录、Registry、安装入口或页面
  - 设计官方插件仓库驱动的初始化内容与打包流程
created_at: 2026-07-13 15
updated_at: 2026-07-13 15
last_verified_at: 2026-07-13 15
decision_policy: verify_before_decision
status: direction_approved_pending_catalog_and_bootstrap_contract
scope:
  - web/app/src/features/settings
  - api/crates/access-control/src/settings_routes.rs
  - api/apps/api-server/src/official_plugin_registry.rs
  - api/apps/api-server/src/official_mcp_bundles.rs
  - api/apps/api-server/src/official_agent_flow_templates.rs
  - /home/taichuy/git/1flowbase-official-plugins
---

# 插件中心统一发现与类型化安装平衡方向

## 谁在做什么

用户与 AI 正在确认跨主仓与官方插件仓库的插件中心方向：在 Settings 首位增加统一发现入口，聚合插件、MCP 配置包和工作流模板，并为后续社区贡献和初始化内容外置建立稳定分发边界。

## 为什么这样做

官方插件仓库已经承载多类可分发产物，但现有入口和 catalog 分散。统一发现可以降低寻找与安装成本；保留类型化安装、预览、导入和管理流程，可以避免把运行时插件、配置包与用户内容错误压成同一种生命周期。

## 为什么要做

主仓应聚焦宿主技术底座和协议不变量，官方插件仓库负责可独立演进的扩展、模板和种子内容，使配置内容调整不必频繁修改主仓，同时为社区按组织贡献产物留下目录和身份边界。

## 截止日期

未指定。

## 已确认边界

- Settings 侧边栏第一项新增“插件中心”。
- 插件中心统一发现与获取，但模型供应商、MCP、应用等领域页面继续负责各自管理。
- 工作流模板现有产品入口继续保留；插件中心可展示同一目录并触发对应使用流程。
- 官方仓库目录逐步采用“类型 / organization / artifact”组织，默认 organization 为 `taichuy`，稳定领域 ID 不因目录变化而重写。
- 初始化采用打包阶段锁定版本与 checksum、随发行物携带、首次启动通过后端领域服务幂等应用；不依赖首次启动拉取不固定的线上 latest。
- 只外置种子内容、模板和可安装扩展；宿主 schema、migration、permissions 和 runtime invariants 继续由主仓维护。
- 模型供应商必须保持向后兼容：现有 `/settings/model-providers` 页面和能力继续保留，不因插件中心而删除、替换或降级；旧 Registry、插件 ID、provider code、Release 资产、已安装记录和实例绑定不得被静默迁移。
- 插件中心允许直接安装模型供应商；供应商安装后的升级、版本切换、实例配置和管理继续进入原模型供应商设置页面。
- 模型供应商现有 `runtime-extensions/model-providers/<provider_id>/` 物理目录本阶段保持不动；新统一 catalog 只将其逻辑投影为 `organization=taichuy`，物理 organization 迁移若有需要必须另行版本化。

## 待继续确认

- 插件中心统一 catalog 的字段、分页和各产物类型投影契约。
- bootstrap profile、发行锁文件、首次启动幂等策略与已有用户内容冲突策略。
