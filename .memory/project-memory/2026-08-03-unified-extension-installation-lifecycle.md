---
memory_type: project
topic: 扩展安装统一生命周期方向
summary: 用户于 2026-08-03 00 确认项目仍处早期阶段，不接受 plugin_installations 与 extension_installations 两套可独立安装、激活、删除的生命周期真值；后续以 extension installation 作为统一聚合根，迁移供应商插件运行与引用关系并删除旧主表和双写路径。允许保留只承载类型专属字段的 1:1 detail/projection 表及节点 artifact instance 表，但它们不得拥有独立生命周期。
keywords:
  - extension-installations
  - plugin-installations
  - unified-lifecycle
  - source-of-truth
  - extension-center
  - model-provider
match_when:
  - 修改扩展中心安装索引或供应商插件版本管理
  - 调整 plugin_installations、extension_installations 或 plugin_artifact_instances
  - 设计插件同步、激活、历史版本或删除语义
  - 重构 Root #1545 的 D2 或 D4
created_at: 2026-08-03 00
updated_at: 2026-08-04 09
last_verified_at: 2026-08-04 09
decision_policy: verify_before_decision
status: direction_confirmed_issue_1566
scope:
  - api/crates/domain/src/extension_installation.rs
  - api/crates/control-plane/src/plugin_management
  - api/crates/control-plane/src/ports
  - api/crates/storage-durable/postgres
  - api/apps/api-server/src/routes/plugins_and_models
  - web/app/src/features/settings
---

# 扩展安装统一生命周期方向

## 谁在做什么

用户与 AI 已将供应商插件纳入扩展中心的方向，从“两套表职责并存并补齐同步”调整为“统一扩展安装生命周期根”。实施与验收由 GitHub Issue #1566 记录。

## 为什么这样做

当前 `plugin_installations` 与 `extension_installations` 都能表达安装版本、来源、可信度、本地状态和生命周期动作，已经产生双写、启动补录、索引缺失与当前版本漂移。项目仍处早期阶段，此时承担一次迁移成本小于长期维护两套状态机的协调成本。

## 为什么要做

让安装、同步、激活、历史版本查询和删除只经过一个聚合根；扩展中心普通读取以数据库为准，不依赖请求期本地目录扫描，并为后续扩展类型复用同一稳定身份与审计边界。

## 截止日期

未指定。

## 已确认边界

- 不再保留 `plugin_installations` 与 `extension_installations` 两套可独立写入和流转的主表。
- 统一聚合根由 `extension_installations` 承担；供应商插件、HostExtension、CapabilityPlugin 都引用该稳定 installation id。
- 节点物理文件状态继续与逻辑安装分层，使用统一 artifact instance 概念；本地路径与节点校验结果不应继续塞进逻辑安装根。
- “当前激活”是 workspace / runtime 消费关系，不是安装版本固有属性；删除资格由后端汇总全部 assignment、instance、task 与系统保留约束决定。
- 采用一次性迁移和切断旧写路径，不保留长期双写兼容；迁移需保全当前 beta 开发库的有效安装及引用关系。
- 普通列表与详情只读数据库；文件系统扫描收口到启动恢复、显式 reconcile 或执行安装 / 删除动作。
- 不预建 `extension_runtime_details`；少量 runtime contract 字段先由统一安装根和现有 projection 承载，出现稳定独立变化后再提取。
- 这是 source of truth 与 migration 变化，按 `grade:g4` Issue #1566 实施并固定迁移、回滚和验收矩阵。

## 2026-08-04 Catalog 身份与搜索契约

- 扩展 catalog 的 canonical identity 使用 `publisher_namespace + plugin_id`；GitHub `@path` 只表示 manifest locator，不参与发布者身份推导。
- 扩展分类使用 `slot_codes`。模型供应商专属入口固定查询 `model_provider` slot，但与通用扩展中心复用同一后端搜索与分页实现。
- 搜索覆盖名称、稳定 identifiers、协议、keywords 与 description；必须先完成搜索和 slot 过滤，再基于同一 verified snapshot/checksum 分页。
- 安装状态只按本地 `extension_installations` canonical exact identity 关联；catalog 安装保留远端 `catalog_id`，上传安装从签名 manifest 的 `publisher_namespace` 构建并校验身份。
- 该契约已于 2026-08-04 在主仓 `dev` 与官方插件仓 `main` 本地合并，并通过官方 catalog、前端/API client 及三 crate Rust 集中回归；是否推送仍由用户人工测试后决定。
