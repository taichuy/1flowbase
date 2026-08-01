---
memory_type: feedback
feedback_category: repository
topic: 扩展安装入口切换必须原地接管历史产物并保留领域页面布局
summary: 成熟插件类型接入扩展中心时，必须为既有本地产物、安装记录与领域绑定提供幂等的 adopt-in-place 迁移；扩展中心集中后端供应链职责，但模型供应商等领域页仍可保留调用同一后端命令的安装、下载、上传、更新和版本切换入口。
keywords:
  - extension-center
  - migration
  - adopt-in-place
  - model-provider
  - settings-layout
  - backward-compatibility
match_when:
  - 把既有插件安装生命周期迁入统一扩展中心
  - 新增或切换 extension inventory 真值投影
  - 调整 /settings/model-providers 或其他领域管理页
  - 验收历史插件、实例、密钥、路由与本地调试产物
created_at: 2026-08-01 15
updated_at: 2026-08-01 16
last_verified_at: 2026-08-01 16
decision_policy: direct_reference
scope:
  - api/plugins
  - api/crates/control-plane/src/plugin_management
  - api/crates/storage-durable/postgres
  - web/app/src/features/settings
---

# 扩展安装入口切换必须原地接管历史产物并保留领域页面布局

## 规则

- 已存在的 `api/plugins` 产物、receipt、`plugin_installations` 与领域绑定必须通过幂等 adopt-in-place 迁移进入新查询投影；不得要求用户重装，也不得静默移动、复制、下载、覆盖或重写本地调试文件。
- 迁移必须保持既有 installation identity、供应商实例、密钥引用、主实例、路由和调用记录；新 inventory 是可重建投影，不得反客为主改变历史运行真值。
- 扩展中心集中的是 catalog、Inventory、风险判断、安装任务和审计等后端供应链职责，不要求成为唯一 UI 入口。
- 模型供应商等领域页面可以继续提供本领域的下载、安装、上传、更新和版本切换入口；这些入口必须调用与扩展中心相同的后端命令和状态投影，不复制 catalog、校验、SemVer、可信度或任务状态机。
- 模型供应商表格可稳定提供“管理 / 新增 / 更新”三个行操作；更新可用性只消费后端 `has_update`，有更新时在更新按钮右上角显示黄色点，无更新时禁用且不显示点，状态列不再重复同一提示。行操作与右侧供应商目录必须复用同一升级确认、兼容性警告、任务和 Inventory 写入口。
- 原领域页面的页面骨架、实例管理、运行状态和当前版本应保留，不能把职责集中解释为整页降级。
- 验收必须包含真实历史 fixture 与实际业务路由；只有全新安装 fixture 或断言旧信息消失的测试不能证明平滑迁移。

## 原因

模型供应商是既有核心扩展类型。Greenfield inventory 与精简页面即使局部测试通过，也可能让本地已有插件在扩展中心不可见，并删除用户已经使用的管理信息结构。

## 停止条件

- 方案需要移动或覆盖既有本地产物。
- 方案会改变供应商实例、密钥、路由或历史安装 ID。
- 原领域页面实现独立于扩展中心后端 contract 的第二套安装、更新判断或状态真值。
