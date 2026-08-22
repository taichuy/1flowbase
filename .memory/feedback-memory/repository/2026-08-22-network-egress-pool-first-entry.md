---
memory_type: feedback
feedback_category: repository
topic: 网络出口以出口池为用户入口，供应方实例是来源边界
summary: 网络中心以代理池为用户入口；最新确认将池收敛为系统唯一全局池。手动代理和扩展解析都在此入口创建，不能强迫用户预先创建供应方实例或派生多个池。
keywords:
  - network egress
  - egress pool
  - static proxy
  - provider instance
  - extension provider
created_at: 2026-08-22 01
updated_at: 2026-08-22 11
last_verified_at: 2026-08-22 11
decision_policy: direct_reference
scope:
  - api/crates/control-plane/src/network_egress_pool.rs
  - api/apps/api-server/src/routes/network_center/pools.rs
  - web/app/src/features/settings/network-center/pools
---

# 网络出口以出口池为用户入口

## 规则

代理池是系统唯一全局出口编排面，也是用户的主入口。添加代理时选择解析类型：内置 HTTP 代理直接填写；扩展类型（如 Clash）填写插件 schema 后，由后端创建实例、解析并把结果加入全局池。代理类型页只展示内置与已安装扩展，不管理实例。

## 原因

用户明确：既有代理本质上就是一个出口，应能从代理池直接添加；Clash 等插件是解析扩展，订阅结果也进入同一系统全局池，而不是另建/选择多个池或要求用户先建实例。

## 适用场景

- 调整网络中心入口、代理创建或扩展同步后的出口编排。
- 设计静态代理、订阅/插件出口与路由选择的 UI/API 语义。

## 已确认的类型页布局

代理类型页沿用模型供应商的双栏信息架构：左侧是可在“新建代理”中选择的解析类型表，右侧只列 `network_egress_provider` 代理插件并提供官方安装与包上传。插件安装属于网络中心的受限系统操作；成功后刷新左侧解析类型表，不复用模型供应商的 workspace assign 流程。

## 运营界面质量要求

代理池不能停留在只有名称、健康和删除的骨架表格。应参考成熟代理管理台提供安全的地址/类型代码展示、筛选和真实的连接测试结果；测试与质量数据必须由后端和运行时能力产出，不能由前端伪造。具体字段及质量检测范围须在下一次计划中冻结。

用户可见的代理类型必须始终使用类型目录中的 `display_name`；`provider_code` 仅是稳定的内部筛选、存储和接口标识，不能在表格与创建表单之间出现不同名称。

代理池每行操作固定为“测试、编辑、删除”；测试操作文案不得写成“测试连接”。编辑仅更新所选代理池成员的单条可编辑状态（启用与顺序），不得重解析、同步或批量修改全局池中的其他成员。
