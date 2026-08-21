---
memory_type: project
topic: 系统备份与恢复 Issue Tree
summary: 用户确认平衡方向：system.backups SettingsFeature、手动完整备份、维护期安全恢复、主库外 manifest/journal、离线恢复入口与受控 Settings UI；BackupSet 记录 source build，target build 从当前二进制编译信息取得，恢复只接受格式、密钥、完整性和已知前向 migration path。线上 Root #1667，Delivery #1668/#1670/#1669。
keywords:
  - system-backup
  - disaster-recovery
  - backupset
  - recovery-job
  - settings-backups
  - issue-1667
match_when:
  - 实现或评估系统备份与恢复
  - 修改 /settings/backups 或 system.backups
  - 设计 BackupSet、RecoveryJob、maintenance fence 或离线恢复
  - 讨论 PostgreSQL、业务对象、插件/MCP 工件的一致备份
created_at: 2026-08-11 23
updated_at: 2026-08-21 12
last_verified_at: 2026-08-21 12
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1667
  - https://github.com/taichuy/1flowbase/issues/1668
  - https://github.com/taichuy/1flowbase/issues/1670
  - https://github.com/taichuy/1flowbase/issues/1669
  - api/
  - web/app/src/features/settings
  - web/packages/api-client
---

# 系统备份与恢复 Issue Tree

## 谁在做什么

- 用户在 `2026-08-11 23` 确认系统备份与恢复采用平衡方向，并要求把完整 Root → Delivery 两层计划挂到线上。
- AI 已创建 Root #1667，以及正式 GitHub 子 Issue：A #1668、B #1670、C #1669；阶段均为 `phase:ready`，尚未进入实现。

## 为什么这样做

- 1flowbase 的 durable data 横跨 PostgreSQL、业务对象存储、插件/HostExtension/MCP 工件；`pg_dump` 不能单独代表可恢复的“系统备份”。
- Web/API 或主数据库不可用时仍需要恢复，因此 BackupSet manifest、operation/recovery journal 与执行入口不能只依赖被恢复数据库或同步 HTTP 请求。
- 恢复会覆盖用户内容、历史状态和审计，必须先做无写预检、安全备份、maintenance fence、worker drain、离线执行、reconcile/health 和 rollback/manual-recovery。

## 为什么要做

- 让 root 管理员在 `/settings/backups` 管理并安全恢复完整系统快照。
- 让同一 BackupSet 在 UI 不可用时仍能由启动期/离线 admin binary 恢复。
- 统一未来升级编排对 backup checkpoint 的消费，避免与已关闭 #344 形成第二套格式或恢复语义。

## 已确认决策与动机

- 独立 `system.backups` SettingsFeature；`restore` 仅 root + recent re-auth，其他动作是独立 console operation。
- v1 只做单节点/自托管、手动全量备份：PostgreSQL + DB 引用业务对象 + 不可重建插件/MCP 工件。
- 排除 external DataSource 远端内容、ephemeral、env/TLS/image 与 BackupRepository 本身。
- BackupSet/manifest 与 RecoveryJob journal 的真值位于主库外；生产仓库与 `api/storage` 分离故障域。
- 接受备份/恢复期间 maintenance 与写暂停；无法形成完整 write inventory 时停止。
- 备份流式、认证加密、内存有界；master key 不入包，只保存 fingerprint。
- 备份 manifest 自动记录 source build；target build 从当前二进制的 `CARGO_PKG_VERSION + Git revision` 取得，不接受用户环境变量声明。
- 恢复 fail closed：必须匹配 format 与 master-key fingerprint、通过完整性校验，且备份 migration head 必须是当前内嵌前向迁移链的已知前缀；build 只用于审计与诊断，不做相等比较。
- HTTP 只创建 intent/观察状态；真正恢复由主 pool 外的 recovery executor/启动期/离线入口执行。
- UI 参考 `/settings/applications` 的 filters/table/toolbar/drawer，但无批量 restore/delete。
- Root 固定 14 个 AC、3 个纵向 Delivery、16 个 Work Packet 和一次集中 Test Batch。

## 截止日期

- 未指定。Root #1667 在全部 AC 有证据、集中 QA 通过、合入 protected baseline 并由用户最终验收前保持有效。
- 若新增 schedule/incremental/PITR、多节点、跨 key/版本迁移、云 snapshot/KMS、双人审批或新的持久数据域，回到 `problem-framing` 并更新 Root。
