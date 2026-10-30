# 系统设置选择性备份

选择性备份区分设置配置与持久业务事实。`storage/durable/postgres` 负责物理表归属、快照、恢复与 rollback，`control-plane-contracts` 维护稳定 selection / category contract；业务授权仍属于控制面与 Canonical Interface。

## 表归属与完整性

`api/crates/storage/durable/postgres/src/system_backup/selective/inventory.rs` 是内置表 feature owner 的唯一实现清单。已登记内置表缺 owner 时拒绝备份，不能忽略表或以“新功能”名义绕过完整性检查。新增持久表时同时登记 owner 与配置 / 数据维度，验证真实 inventory、source 和 preflight，保留非目标行与 rollback 证据。

通知实例的 `notification_instances` 属于 `system.notifications` 的配置；收件人的 `system_notifications` 是同 owner 的持久 inbox 历史。仅导出配置不包含收件历史，选择数据时沿用 selection contract 组合所需配置。通知实例 credential 密文使用 notification repository 的 workspace / instance associated data 验证身份，并在恢复时按现有 selective rekey 流程换到目标 key；无配置凭证的内置实例保留 null。不得直接搬运源密文、丢失身份绑定或输出明文。备份 owner 不授予普通数据表访问权限，通知 service 仍独占接收人隔离与业务读写。

供应商会话归属 `model_provider_session_bindings` 是 `system.model-providers` 的持久运行数据，不属于配置。仅配置备份不包含它；选择数据时保存和恢复精确 scope、实例归属及创建时间。实例已删除时仍保留归属，不因缺实例而丢弃；恢复保留目标独有绑定并支持 rollback，后续正常 get-or-bind 仍不能覆盖恢复的原实例。

## Identity 列恢复

`selective/identity.rs` 从目标 PostgreSQL catalog 发现本次选择表的 identity 列和所属序列。恢复事务锁定表与序列，临时将 ALWAYS 列切为 BY DEFAULT，沿用原 upsert 恢复精确列值；延迟约束完成检查后恢复原生成模式。Prepared 的 commit / rollback 同时决定行、生成模式与序列状态，不能提前提交模式变更或用非事务性的 setval 替代。

序列恢复按增量方向取现有分配高水位与完整目标表极值的较大进度，再约束到原序列边界。事务内 RESTART 配合临时 CACHE 1 标记已占用边界，随后恢复原 CACHE，避免重放不断预留新批次。保留正负增量、原生 CYCLE / exhaustion 与目标独有行，不重编号归档身份、不扩大归档格式。非循环序列的后续分配越过已恢复极值或正常耗尽；循环序列保留原有回绕及唯一约束行为，不承诺无间隙或并发任意显式 identity / schema DDL 的序列化结果。

## 历史迁移验收

历史迁移 fixture 固定迁移前的 schema，使用当时存在的 workspace 行契约建立初始数据。当前 bootstrap 会为新增 feature 建默认实例，因此不能用于通知表尚不存在的历史数据库；测试通过 `postgres-test-support::insert_historical_workspace` 只插入基础行，不补未来 migration、不放宽 rollback 断言。正式 migrations 的版本、顺序和内容保持不变。

## 证据边界

分类正反例与当前完整迁移库验证 owner 覆盖；历史库验证真实升级与失败回滚。结构扫描、单表存在或测试编译成功不等于备份恢复行为通过；证据绑定当前候选 SHA 与线上 artifact。
