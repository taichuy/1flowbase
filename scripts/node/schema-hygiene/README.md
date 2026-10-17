# Schema Hygiene Rules

## Profiles

- `managed_table` is the default for every unmarked physical table.
- `dynamic_model_table` is for generated workspace-scoped user data tables.
- `registered_system_table` is a fixed physical table registered into metadata.

## registered_system_table

`registered_system_table` means fixed physical table, metadata registration, and read-only field template.

It is not a schema hygiene exemption. The scanner still checks required physical columns, primary key shape, scope, indexes, constraints, and parse failures. If an existing registered system table misses a fixed-template requirement, the gate must report the difference first; schema repair belongs in a separate migration issue.

Metadata systems may manage display configuration, actions, views, and relation metadata for these tables, but must not add, delete, rename, or physically change registered system table columns.

## Owner-keyed operational tables

`model_provider_session_bindings` 按 `(workspace_id, scope_digest)` 保存不可替换的会话发起实例，不保存正文，也不使用普通时间分页。窄 owner contract 保留 workspace 外键、非空摘要和实例身份、组合主键及查询索引；实例删除不级联清除绑定，避免默认路由静默换号。通用 owner 正反例同时验证该表，不豁免创建时间或 owner 约束。

日志删除任务、固定删除工作集、停止事实和上传幂等回执按 application/job/event 身份查找；组织树及绑定按 workspace/parent/member 关系查找。它们不消费普通业务表的 scope chronology。`config.json` 只豁免不适用的具体字段/索引规则，`ownedTableContracts` 同时保留主键、非空 owner 外键与实际查询索引检查。合法树根允许 nullable `parent_id`；普通业务表仍要求 scope/time/id 索引。

原生消息投影进度复用 `flow_run_owned_table`：一个 run 一行、非空 flow_run 外键、主键索引，不能通过普通 exemption 掩盖丢失的 run owner。规则正例来自正式 migration inventory，受控反例移除主键、外键、owner 非空或查询索引时必须失败。

插件出站凭据按 `(publisher_namespace, plugin_code, scope_id, credential_id)` 精确查找，不虚构独立行 ID 或时间分页索引；仍检查 scope 与创建/更新生命周期字段。`plugin_schema_version_objects` 是 `(owner_id, owner_version, ownership_key)` 不可变版本成员集合，继续验证 ownership catalog 外键及 owner/version 查询前缀。两者使用同一 `ownedTableContracts`，不通过删门禁放行错误身份或丢失 owner。


通知配置与 inbox 是专用 owner-keyed operational 数据：实例按 workspace/created_at/id 查询，历史按 workspace/recipient/created_at/id 查询并通过 read_at 确认。只豁免不消费的通用 scope chronology 和不适用的 updated_at；`ownedTableContracts` 仍要求实例 workspace 外键、历史非空 workspace/recipient/instance 身份、read_at 与真实查询索引。历史特意不以用户/实例外键级联删除；没有 scope_id 不作为普通业务表 readiness 的替代，未将通知表伪装成 system/global 或 append-only。

`notification_send_logs` 同样使用窄 owner contract：每次发送一个 workspace-owned attempt，repository 仅将 pending 更新为 terminal 并写 completed_at；并非 append-only，也不是 system/global。豁免只覆盖通用 scope chronology 与可编辑 updated_at；保留 id/created_at 检查，并验证非空 workspace/actor/instance、status/source/completed_at 与两种实际 workspace 查询索引。无 actor/instance 外键是保留历史身份的既有契约。正例来自正式 migration，负例删除 owner、生命周期列或任一查询索引仍须失败。
