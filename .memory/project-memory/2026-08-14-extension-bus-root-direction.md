---
memory_type: project
topic: Extension Bus 架构方向与线上 Root 已建立
summary: 用户已确认 1flowbase 插件底座采用“控制平面 + 类型化执行通道”的 Extension Bus；全局扩展点第一阶段仅由 Boot Core 与 Trusted HostExtension 定义，线上计划真值为 Root #1688 及 Delivery #1689-#1693，当前仍处于 phase:discussion，须批准 Root 完整范围与 AC 后才实施。
keywords:
  - extension-bus
  - plugin
  - module-descriptor
  - effective-extension-graph
  - lifecycle
  - typed-lanes
  - issue-1688
match_when:
  - 继续规划或实现插件架构、模块自注册、扩展点或插件生命周期
  - 需要判断 Extension Bus 已确认的授权与安全边界
  - 需要定位当前线上插件架构计划入口
created_at: 2026-08-14 09
updated_at: 2026-08-14 09
last_verified_at: 2026-08-14 09
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1688
  - https://github.com/taichuy/1flowbase/issues/1689
  - https://github.com/taichuy/1flowbase/issues/1690
  - https://github.com/taichuy/1flowbase/issues/1691
  - https://github.com/taichuy/1flowbase/issues/1692
  - https://github.com/taichuy/1flowbase/issues/1693
  - api
  - web
  - plugins
---

# Extension Bus 架构方向与线上 Root 已建立

## 时间

`2026-08-14 09`

## 谁在做什么

- 用户确认把现有插件治理骨架提升为统一 Extension Bus，而不是只新增一个 Event Bus 或继续为现有业务域堆专用 Registry。
- Root #1688 是唯一线上计划、进度和用户验收真值；#1689-#1693 是 GitHub 原生子 Issue，对应五个纵向 Delivery。
- 当前只完成计划创建，所有 Issue 保持 `phase:discussion`；用户批准 Root 正文后，才进入只读 Scout、packetization 与开发。

## 为什么这样做

- 当前已有 Manifest、安装/制品/运行状态以及多个领域 Registry，但缺少模块声明扩展点、插件声明贡献、统一编译/激活/解释/回收的闭环。
- 未来新增业务模块需要复用稳定范式自行导出 Descriptor，而不是修改 Extension Bus 内核或在 API Server 启动代码中增加专用扫描路径。
- 总线必须吸收跨模块的依赖、冲突、作用域、生命周期和 provenance；领域模块继续拥有强类型 contract、权限、不变量与持久化。

## 已确认决策

1. Extension Bus 由控制平面与类型化执行通道组成；执行通道首期为 `slot`、`pipeline`、`event_stream`、`contribution`、`resource_action`。
2. 只有 Boot Core 与 Trusted HostExtension 可以定义全局扩展点；RuntimeExtension、CapabilityPlugin 和用户低代码只能向已授权扩展点提供实现、贡献或监听。
3. 插件自定义子扩展点不是第一阶段范围；未来如开放，只能位于自身 namespace，生命周期嵌套于父插件且不得权限升级。
4. HostExtension 继续 trusted、boot-time、restart-scoped，不支持 Rust native `so/dll` 反复热卸载。
5. `extension_installations` 继续作为统一安装生命周期真相；Effective Extension Graph 只是可哈希、可解释的派生结果。
6. Extension Bus 不得退化为动态 JSON 总线、万能 Hook、第二业务层或绕过领域 Service/鉴权/持久化的入口。

## 为什么要做

- 让 Application/Workflow、Interface HTTP/MCP、后端数据源与低代码在共享插件范式下扩展，同时保留各自的业务和安全边界。
- 让未来模块可以通过 Descriptor 自注册扩展契约，并让启动、Registry、诊断与 Extension Center 消费同一 Effective Extension Graph。
- 为 Host restart、Runtime process/worker、Workspace/UI mount/realm 提供符合各自语言与资源模型的回收边界。

## 截止日期

- 未指定。

## 线上计划

- Root #1688：模块自注册、类型化执行与完整生命周期的 Extension Bus。
- D1 #1689：模块自注册、Effective Graph 与真实 Host Provider 同源启动。
- D2 #1690：Runtime Provider 类型化调度与进程生命周期闭环。
- D3 #1691：Application 与 Workflow 受控介入和分级观察闭环。
- D4 #1692：Interface Operation 向 HTTP 与 MCP 同源投影。
- D5 #1693：低代码贡献注册与可终止隔离 Realm 生命周期。

## 停止条件

- 总线只是现有 Registry 的转发包装，Consumer 仍识别具体插件或读取通用 Manifest JSON。
- 需要开放普通插件定义全局扩展点、改变统一安装真相、引入用户数据迁移或支持 Rust native 热卸载。
- 无法用一个真实 Host Provider 证明声明、启动、Consumer 与 effective-plan dump 消费同一图。
