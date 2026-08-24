---
memory_type: project
topic: UI 组件目录从 export override 转为独立持久化记录
summary: Root #1851 已完成 D1–D4、两轮集中 QA 与双仓合并，当前进入人工验收；官方组件来自外置 catalog，主后端只持久化与消费独立记录，不扫描 exports 或校验代码可执行性。
keywords:
  - ui-management
  - component-catalog
  - module-exports
  - insert-snippet
  - structured-form
created_at: 2026-08-21 22
updated_at: 2026-08-24 10
last_verified_at: 2026-08-24 02
decision_policy: verify_before_decision
source_issue: "#1851"
scope:
  - api/crates/control-plane/src/ui_management.rs
  - api/apps/api-server/src/routes/frontstage/component_capabilities.rs
  - api/apps/api-server/src/routes/settings/ui_management.rs
  - web/app/src/features/settings/components/ui-management
  - web/app/src/features/frontstage/components/jsx-studio
---

# UI 组件目录契约边界交付

## 2026-08-24 扩展中心目录投影

- 扩展中心新增 `/settings/extension-center/ui-components`，作为同一官方 UI Component Catalog 的发现、搜索、分页、下载、检查更新和按组同步入口。
- 该入口复用现有 UI catalog API 与 `ui_component_records` 本地状态，不写入 `extension_installations`，不提供启用、停用、卸载或运行时安装语义。
- `/settings/ui-management/components` 继续拥有本地记录和自定义 CRUD；扩展中心提供“前往 UI 管理”入口。
- 前端目录主体由扩展中心页面与 UI 管理抽屉共享；主后端没有新增写死组件、存储表或接口。
- 已以提交 `7aaf7d270` 开发，并由 merge commit `9581fc216` 合入 `dev`；聚焦测试 40/40、TypeScript、定向 ESLint 和受保护页面快照通过。

## 2026-08-23 产品语义纠正

用户明确否定“遍历已安装模块 exports，再用 override 补契约”作为组件管理主模型。新的稳定方向是：

- UI 组件是后端数据库中的独立纯记录，不以 module export 为存在前提。
- 插件仓库提供一组组件记录，平台可主动拉取并成组同步更新。
- 用户可自行新建、编辑自己的组件记录。
- 记录包含原始 import 代码片段和组件插入代码，区块编辑器只消费持久化组件目录。
- 官方样板固定来自 `/home/taichuy/git/1flowbase-official-plugins/ui_components`，发布和分页 catalog 结构参考同仓 `i18n`。
- 主后端只负责 catalog 查询、下载、持久化和消费；默认尝试拉取 `taichuy` 官方源，失败不阻断主程序。
- 组件是否可用由仓库维护者负责；同步链路不做 import 可执行性校验，不新建“可解析/不可解析”状态。误收录时由维护者更新或删除；区块编译器已有的通用错误处理不属于组件同步职责。

此纠正 supersede 下文原“已批准边界”中以官方 manifest 组件契约 + export override 为组件目录主体的部分；原交付证据仅作历史背景。

## 2026-08-23 新计划真值

- 旧 Single Issue #1823 已写入 superseded 评论并以 `not planned` 关闭。
- 新 Root #1851 是唯一活动计划。
- Delivery：#1854 官方 catalog 供应链；#1853 独立记录与 settings CRUD；#1855 默认源与成组同步；#1852 区块编辑器消费持久化记录。
- Root 与 D1–D4 已于 2026-08-23 获用户一次性授权并进入 `phase:implementation`。
- protected baseline：主仓 `dev` 为 `29ab278e8937b436f075a8eaa2affcb8ae5b3bc4`；官方仓 `main` 为 `107873a73f553aaf57aa9875076d6ac7fad6d7a9`。
- 冻结 Work Packet：D1 官方 catalog；D2 独立持久化与 settings CRUD；D3 source/group 同步与 best-effort bootstrap；D4 JSX Studio 持久化目录消费；最后由一个 fresh QA 对冻结 assembly 集中验收。

## 谁在做什么

Root agent 是唯一 packetizer、assembly owner 与 Control Ledger owner。D1–D4 已完成并合入：主仓 `dev` merge commit `b7a8fdf9b`，并发接口 inventory 集成修复 `8623b0eac`；官方仓 `main` merge commit `f80332e`。当前等待用户人工验收。

## 2026-08-24 最终交付证据

- D1 official catalog：`f4c81be18aa58fb5bc6ae1a3111b42da85740988`。
- 主仓冻结 assembly：`c61cf2b74a91`；第二轮 fresh QA 为 `QA_PASS`，无 blocker/high finding。
- 自动化：official 16/16；domain 1/1；control-plane 6/6；storage-postgres 3/3；plugin-framework 1/1；api-server catalog/CRUD/ACL/Frontstage/dependency-lock/route/inventory 全部通过；api-client 244/244；focused app 38/38；i18n 0 error。
- 运行态：隔离数据库中真实创建 page/tab/native JSX block/组件，settings 与 Frontstage JSX Studio 的桌面中文、移动英文均通过；默认 catalog 拉取失败不阻断 `/health`。
- 合并后 `dev` 因并发网络功能新增 family uninstall 接口，exact inventory 从 360 更新为 361；真实 `dev` 上同一精确断言 1/1 通过。
- QA 证据位于 `tmp/test-governance/ui-component-catalog-root/qa-cycle-2/`；临时服务、session、fixtures 与数据库均已清理。

## 为什么这样做

提交 `53df6afe9` 将所有已注册 module export 暴露为组件候选，并在缺少契约时插入裸 `export_name`，把可 import 符号与可插入组件混为一谈。后台组件契约又默认使用原始 JSON 编辑，偏离结构化管理预期。

## 已批准边界

- 官方 manifest 是模板、模块和组件契约代码真值；`inherit` 随官方更新，用户 published override 独立持久化。
- Frontstage 组件目录只展示完整有效契约；无契约 export 仍可通过源码静态 import 使用。
- 后台使用结构化表单管理契约，并允许为已安装、当前节点可执行的 export 新建用户契约。
- 不包含在后台编写、编译或发布新的运行时组件实现，不做历史数据清理或 migration。
- 不关闭 Issue，保持 `phase:user-acceptance`，等待用户独立审核。

## 截止与验收

线上 issue #1823 的 AC-001～AC-008 是唯一计划与验收真值。2026-08-21 已完成集中 QA：定向前端 11/11、api-server 3/3、control-plane 2/2、Rust 静态门禁与桌面/移动受保护页面取证通过；仅保留仓库既有 TypeScript build 错误与 140 条 i18n unused-key warning 作为非本次阻断风险。用户验收反馈后，Props、限制、示例和上游来源已进一步收敛为单列字段流（提交 `b8b6309b`），定向前端 6/6 与桌面/移动运行态均通过。
