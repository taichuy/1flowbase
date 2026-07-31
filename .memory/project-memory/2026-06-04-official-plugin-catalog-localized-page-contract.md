---
memory_type: project
topic: 官方插件安装目录轻量本地化分页契约
summary: 用户确认官方插件安装目录由后端返回轻量、本地化、可搜索、可分页的列表 DTO；并于 `2026-07-31 00` 确认本地已安装供应商查询必须与上游 Registry 解耦，官方目录采用有界超时、singleflight 与 stale-while-refresh。
keywords:
  - official-plugin
  - official-catalog
  - i18n
  - pagination
  - settings
  - model-provider
match_when:
  - 需要调整官方插件安装目录接口
  - 需要处理官方插件列表多语言切换
  - 需要判断插件 i18n bundle 应由前端还是后端解析
  - 需要优化官方插件 registry 列表加载和搜索
created_at: 2026-06-04 12
updated_at: 2026-07-31 00
last_verified_at: 2026-07-31 00
decision_policy: verify_before_decision
scope:
  - api/apps/api-server/src/routes/plugins_and_models/plugins.rs
  - api/crates/control-plane/src/plugin_management/catalog.rs
  - api/apps/api-server/src/official_plugin_registry.rs
  - web/app/src/features/settings/api/plugins.ts
  - web/app/src/features/settings/components/model-providers/OfficialPluginInstallPanel.tsx
  - ../1flowbase-official-plugins/official-registry.json
---

# 官方插件安装目录轻量本地化分页契约

## 时间

`2026-06-04 12`

## 谁在做什么

- 用户确认将官方插件安装目录从“前端拿完整 i18n catalog 后自行解析”调整为“后端返回列表页可直接展示的本地化 DTO”。
- AI 已按该方向在主仓实现官方目录轻量列表响应、locale/search/limit/cursor 参数、前端 locale query key 和官方 registry 短 TTL 缓存。

## 为什么这样做

- 插件数量增多后，列表页不应该为展示名称和说明拉取完整插件 `fields/parameters/options` 等大 JSON。
- 前端解析插件 `i18n_catalog` 会让 UI 知道插件 bundle 内部 key 结构，导致语言切换、缓存和接口边界都变脆。
- 用户安装插件时最需要快速搜索可安装供应商，列表接口应优先返回系统可安装 artifact、本地化名称/说明和安装状态。

## 为什么要做

- 降低 `/settings/model-providers` 官方安装区加载成本。
- 让语言切换时官方插件名称、说明和 source label 跟随当前 locale。
- 给后续插件市场分页缓存、搜索和详情按需加载留下稳定契约。

## 截止日期

- 无

## 决策背后动机

- 官方安装目录接口返回轻量列表字段：`plugin_id`、`provider_code`、`display_name`、`description`、`icon`、`protocol`、`latest_version`、`selected_artifact`、`help_url`、`model_discovery_mode`、`install_status`、`page`。
- 官方安装目录列表响应不再返回插件 `i18n_catalog`。
- 后端负责 locale fallback、source label 本地化、搜索过滤、分页 cursor 和系统安装 artifact 选择。
- 前端 query key 必须包含当前 locale 和官方目录搜索词；前端不再解析官方插件 i18n bundle。
- 官方 registry adapter 可以短 TTL 缓存 registry document；安装包下载仍按安装动作独立下载，不混入列表缓存。

## `2026-07-31 00` 韧性补充

- 用户确认并已完成平衡方向：本地 `families` 只读取本地数据与已有官方快照，不得因官方 Registry 网络请求阻塞。
- 官方目录冷启动请求使用连接与总请求有界超时；同一刷新窗口最多一个上游请求。
- 有旧快照时立即返回并标记 `source_freshness=stale`，后台刷新；无快照且上游失败时由右侧官方目录面板显示本地化提示和重新加载动作。
- 原始网关 HTML 只保留在 API client 的诊断 body，不作为用户可见错误文案，也不在页面顶部重复报错。
