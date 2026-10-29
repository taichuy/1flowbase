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

通讯配置保留 `instance_id` 单行渠道身份，通过 `(scope_id, instance_id)` 外键核对既有通知实例 workspace；只豁免额外 `id` 与不适用的 id 结尾通用索引，并验证真实 `(scope_id, created_at, instance_id)` 索引。`20261026160000_communication_timeline.sql` 将原处理信封改名为 `communication_inputs`，删除其会话外键，迁移后删除旧附件表；新的 `communication_messages` 保存 typed text/file timeline。两表保留普通物理 scope、创建/更新时间及完整发送者/会话身份，由 repository 的完整 scope 条件校验历史归属，不要求不存在的生命周期外键。处理状态、发送状态、claim 与 admission 属于 inputs；type、direction、发生时间与文件恢复字段属于 timeline。原 messages 更新时间 trigger 跟随改名后的 inputs，timeline 使用独立 trigger。规范来源为该迁移、`20261028120000_communication_sessions.sql`、`docs/architecture/communications/README.md` 与 communications repository。

`runtime_internal_compaction_contents` 以 `(scope_id, application_id, session_key, content_id, content_kind)` 保存不可变 canonical body 所有权，`runtime_internal_compaction_heads` 以前三列保存可变 head。窄豁免只覆盖独立 id、普通 chronology 与各自不适用的更新时间/创建时间；不添加虚构业务列。owner contract 继续验证实际组合主键、非空 session 路由、application/canonical content 复合外键、body 反查索引与真实生命周期字段。checkpoint 引用合法可空；head 的 canonical content 引用保留默认 NO ACTION，不能误改为级联删除。规范来源为 `20261027120000_native_internal_compaction.sql` 与 `orchestration_runtime_repository/context_storage/internal_compaction_methods.rs`。
