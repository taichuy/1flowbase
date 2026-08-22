---
memory_type: project
topic: system backup 仅保留已分配模型提供方版本
summary: 用户于 2026-08-22 确认：模型提供方插件备份必须按 workspace assignment 判断实际使用版本；历史 active_requested 记录但没有 assignment 的版本不应阻塞备份。修复已合入 dev 的 6f34096f3。
keywords:
  - system-backup
  - plugin
  - model-provider
  - assignment
  - inventory
match_when:
  - 排查 system_backup_source_inventory_invalid
  - 调整模型提供方安装、版本切换或备份清单
created_at: 2026-08-22 18
updated_at: 2026-08-22 18
last_verified_at: 2026-08-22 18
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/plugin_management/backup_export.rs
  - api/crates/control-plane/src/plugin_management/install.rs
---

# system backup 仅保留已分配模型提供方版本

用户确认的恢复语义是：model provider 的可恢复性以 workspace assignment 为准，而不是仅以安装记录的 `desired_state=active_requested` 为准。版本切换会迁移实例与 assignment，但可能保留旧记录的 desired state；该旧版本并不参与运行时，也不应因其保留包已清理而阻塞备份。

`backup_export.rs` 复用 `PluginRepository::list_assigned_installation_ids()`：只有 `active_requested` 或 `pending_restart` 且仍被 assignment 引用的 model provider 才会进入备份。其它插件类别维持原有激活状态规则。对这些已分配模型提供方，当前节点 `ready` 制品即使 `is_current=false` 也可作为恢复源；未分配模型提供方的当前制品仍会先从实例索引移除，避免误报 orphan artifact。

2026-08-22 18：定向 `backup_export_tests` 14/14 通过，Rust static gate 无 warning。首次禁用增量的红测在资源争用下被资源管理器以 SIGTERM 终止，未形成红灯证据；后续增量绿测覆盖了已确认行为。

2026-08-22 19：第二个真实错误证明上次只覆盖了历史版本筛选，未覆盖当前节点 `ready + is_current=false` 的已分配版本。`f2c72315d` 增加正反回归用例；定向 `backup_export_tests` 16/16 通过，静态门禁无 warning。
