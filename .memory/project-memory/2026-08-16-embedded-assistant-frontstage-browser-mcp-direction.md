---
memory_type: project
topic: 内置助手 Frontstage 浏览器操作 MCP 方向
summary: 用户确认采用 client-bound MCP，并已建立 GitHub Root #1738 与 Delivery #1739-#1742：只供正在显示并通过当前回答 WebSocket 连接的内置 AI 助手使用，不暴露给外部 MCP。2026-08-17 生命周期进一步收敛为随包模板保留不可变真值、workspace 首次导入一次后自由编辑删除、对应实例在行内更多菜单显式预览并恢复默认；重启和升级不得覆盖 workspace 修改。
keywords:
  - embedded assistant
  - frontstage
  - browser client tools
  - client-bound MCP
  - websocket lease
  - block render inspection
  - block click
  - block recompile
  - extension center
match_when:
  - 规划或实现内置助手读取、搜索、点击或重编译当前页面区块
  - 调整浏览器客户端工具与 MCP 配置、扩展中心模板的边界
  - 判断助手窗口关闭、WebSocket 断开或重新附加运行时的前端工具可用性
created_at: 2026-08-16 00
updated_at: 2026-08-17 15
last_verified_at: 2026-08-17 15
decision_policy: verify_before_decision
status: root_ready
source_issue: "#1738"
delivery_issues:
  - "#1739"
  - "#1740"
  - "#1741"
  - "#1742"
scope:
  - web/app/src/app-shell
  - web/app/src/features/agent-flow
  - web/app/src/features/frontstage
  - web/packages/page-runtime
  - web/packages/api-client
  - api/apps/api-server/src/routes/assistant
  - api/crates/domain/src/mcp_management.rs
  - api/crates/control-plane/src/mcp_bundle.rs
---

# 内置助手 Frontstage 浏览器操作 MCP

## 谁在做什么

内置助手通过当前回答的 WebSocket 请求后端，后端再把 client-bound Tool 调用发送给正在显示助手的同一浏览器标签页；Frontstage Runtime 提供区块运行状态、受限 DOM 投影、节点引用、点击和单区块重编译。MCP Bundle 只管理工具目录、描述、分组、状态、映射、风险和恢复模板，不把浏览器伪装成可供外部客户端连接的 MCP Server。

## 为什么这样做

浏览器 DOM、ShadowRoot、当前区块实例和点击能力只存在于活动标签页。后端无法在没有活动 WebSocket 与前端能力声明时观察或控制这些状态，因此工具仅在助手可见、当前回答 WebSocket 已连接且目标页面 Runtime 已挂载时有效。

## 已确认边界

- 不向外部 MCP 客户端暴露；无当前助手 WebSocket 时不可调用。
- 助手关闭、标签页离开、workspace 切换或 WebSocket 断开时立即撤销本轮 client capability；等待中的调用 fail-visible。
- 模板随发行包离线可用并初始化到扩展中心；随包文件是不可变恢复真值。
- workspace 只在从未导入时自动 seed 一次；此后实例、Tool、目录、绑定和发现策略都属于可自由编辑删除的 workspace 内容，API 重启和升级不得覆盖或重建。
- `managed_by` 当前只投影 Bundle 来源，不再构成写保护。恢复默认是实例级动作，入口只放在对应实例的行内“更多操作”中。
- 自动初始化只覆盖 MCP 配置图，不自动创建 Frontstage 业务区块。
- 读取结果必须有长度上限、cursor 与 render/node 引用；点击不接受任意 selector、坐标或脚本，旧实例引用必须拒绝。
- 重新编译只推进目标区块 generation，不重新编译或 remount 相邻区块；目标输出变化引起的既有下游 Signal 更新仍允许发生。

## 当前计划状态

产品方向与两层 Issue Tree 均已批准并写入 GitHub：Root #1738 是唯一计划、Control Ledger 与最终用户验收真值，四个原生 sub-issue 为 #1739 Client-bound MCP 租约、#1740 区块观察与引用、#1741 受控点击与局部重编译、#1742 随包模板与恢复。全部 Issue 当前为 `phase:ready`；执行从 Root 的只读 Scout、一次 packetization 与集中 Test Batch 开始。

## 2026-08-17 MCP 示例生命周期确认

- 用户确认采用平衡方向：`frontstage_assistant` 首次 seed 一次，之后 workspace 可自由编辑和删除。
- 首次 seed 与 durable import receipt 在同一事务提交；已有历史受管数据通过 migration 补 receipt，避免升级时再覆盖一次。
- 模板库继续持有不可变默认包；对应实例恢复必须先 preview，再由用户确认，恢复后的资源仍保持可编辑。
- “配置包来源”只表达 provenance；不得再用它禁用前端操作或让 control-plane 返回 `mcp_system_managed`。
- 用户纠正恢复入口归属：不得在列表顶部放“恢复默认”，避免形成恢复全部的语义；只在有配置包来源的对应实例“更多操作”中显示“恢复此实例默认”。

## 2026-08-17 扩展中心恢复入口确认

- 用户确认扩展中心是实例被删除后的标准恢复入口；MCP 管理行内“更多操作”保留为同一实例级 contract 的快捷入口。
- `frontstage_assistant` 必须作为内置、离线可用模板合并进扩展中心 MCP catalog；远程 catalog 即使没有该条目也必须可见。
- 扩展中心“查看”按模板实例展示 workspace 状态，并对每个实例提供“恢复默认”；不得恢复整个 Bundle，也不得在 MCP 管理列表伪造已删除实例。
- 恢复请求直接使用随代码发布的不可变备份真值 `builtin_template_id + instance_id`，不依赖历史 import receipt 或 Extension Installation 索引；删除后仍能恢复。
