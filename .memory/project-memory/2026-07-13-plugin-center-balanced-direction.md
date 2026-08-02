---
memory_type: project
topic: 插件中心统一发现与类型化安装平衡方向
summary: 用户确认扩展中心平衡方向并已建立线上两层 Issue Tree #1545：统一六类官方目录与分页 catalog、本地扩展真值与安装 inventory、类型化应用与非阻塞 bootstrap、扩展中心及模型供应商安装集中管理；来源与可信度分列，签名异常只告警，本地产物存在时不得自动远程修复或替换。
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
updated_at: 2026-08-03 00
last_verified_at: 2026-08-03 00
decision_policy: verify_before_decision
status: superseded_by_unified_extension_lifecycle
scope:
  - web/app/src/features/settings
  - api/crates/access-control/src/settings_routes.rs
  - api/apps/api-server/src/official_plugin_registry.rs
  - api/apps/api-server/src/official_mcp_bundles.rs
  - api/apps/api-server/src/official_agent_flow_templates.rs
  - /home/taichuy/git/1flowbase-official-plugins
---

# 插件中心统一发现与类型化安装平衡方向

> Superseded at `2026-08-03 00`: 用户基于项目仍处早期阶段，否决继续保留
> `plugin_installations` 与 `extension_installations` 两套安装生命周期真值；后续以
> `.memory/project-memory/2026-08-03-unified-extension-installation-lifecycle.md`
> 记录的统一生命周期方向为准。本文仅保留此前边界与 Issue Tree 证据。

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
- 模型供应商插件的发现、官方目录获取、本地上传、安装、更新检测以及安装 / 更新任务统一收口扩展中心；原 `/settings/model-providers` 页面继续负责供应商实例、密钥与参数、模型目录、路由、调用记录等领域管理，不再维护平行的远端安装目录。
- 扩展中心表格把“来源”与“可信度”拆成两列：来源表达 `builtin / official_registry / mirror_registry / uploaded` 等获取渠道，可信度表达“官方 / 可信 / 未知”，不能再用一个字段混合两种语义。
- 签名缺失、签名密钥未知或签名存在但验签失败都不硬阻止安装；后端保留具体 `signature_status`、把可信度显示为未知，UI 明确警告，用户确认后可继续，并记录 override / 审计。
- `api/plugins` 已存在本地产物时，本地产物是安装与调试的唯一依据；签名或 checksum 异常只记录和告警，不触发任何远端备份下载、自动修复或替换。只有本地缺失且既定 bootstrap / 用户安装动作明确要求获取时才访问远端。
- 产物下载到宿主 `api/plugins` 与 workspace 使用分层：宿主只安装一次；模型供应商实例、CapabilityPlugin 分配、MCP 导入、Agent Flow 创建与 i18n 激活继续由目标 workspace 的领域服务和权限控制。用户已确认该边界。
- 模型供应商现有 `runtime-extensions/model-providers/<provider_id>/` 物理目录本阶段保持不动；新统一 catalog 只将其逻辑投影为 `organization=taichuy`，物理 organization 迁移若有需要必须另行版本化。

## 线上 Issue Tree

- Root：[#1545 统一官方扩展仓库目录、扩展中心与本地安装生命周期](https://github.com/taichuy/1flowbase/issues/1545)
- D1：[#1546 统一六类官方扩展目录与静态分页 Catalog](https://github.com/taichuy/1flowbase/issues/1546)
- D2：[#1549 建立本地扩展真值、Catalog Gateway 与安装 Inventory](https://github.com/taichuy/1flowbase/issues/1549)
- D3：[#1548 建立类型化应用与非阻塞默认扩展 Bootstrap](https://github.com/taichuy/1flowbase/issues/1548)
- D4：[#1547 交付扩展中心并集中模型供应商插件安装管理](https://github.com/taichuy/1flowbase/issues/1547)
- GitHub 原生 sub-issue 关系已建立，全部处于 `phase:ready`；后续以 Root 正文和 Control Ledger 为唯一计划真值。
