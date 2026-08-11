# ADR: 系统备份与恢复 contract

- 状态：Accepted
- 日期：2026-08-11
- 关联 Issue：[Root #1667](https://github.com/taichuy/1flowbase/issues/1667)、[#1668](https://github.com/taichuy/1flowbase/issues/1668)、[#1670](https://github.com/taichuy/1flowbase/issues/1670)、[#1669](https://github.com/taichuy/1flowbase/issues/1669)
- 任务形态：`hybrid-foundation`

## Context

1flowbase 的 durable system state 同时存在于 PostgreSQL、数据库引用的业务对象，以及插件、HostExtension 与 MCP 工件。单独执行 `pg_dump` 无法证明系统可以恢复。恢复又可能令主数据库和 Web/API 暂时不可用，因此 BackupSet 目录、operation journal、RecoveryJob journal 与恢复执行入口不能以被恢复数据库为唯一依赖。

## Decision

### Canonical objects

- `BackupSet` 是 seal 后不可变的成果；availability 为 `ready / corrupt / incompatible`。创建中与验证中属于独立 `BackupJob`。
- `BackupManifest` 固定记录 format、build、migration head、master-key/backup-key fingerprint、component inventory、digest、size、流式上限和明确排除域。
- `RecoveryJob` 独立记录 `preflight → confirmation → safety backup → fence/drain → restore → reconcile → verify`，终态为 `succeeded / rolled_back / manual_recovery_required`。
- BackupSet、BackupJob journal 与 RecoveryJob journal 的真值位于主库外的 `BackupRepository`；主库只保存恢复成功后的审计投影。

### Included and excluded domains

v1 只支持单节点、自托管、手动全量备份。每个 ready BackupSet 必须包含 PostgreSQL、数据库有限 inventory 引用的业务对象、以及不可重新下载或重建的插件/HostExtension/MCP 工件。官方可重建工件仍记录 identity 与 digest，可以不内嵌 payload。

明确排除 external DataSource 远端内容、session/cache/queue 等 ephemeral state、环境变量、TLS material、容器镜像与 BackupRepository 自身。`runtime_debug_artifacts` 只要仍由 durable database record 引用，就按业务对象纳入；若要改为 ephemeral，必须另行改变其领域 contract。

### Integrity, encryption, and compatibility

- envelope 使用固定 4 MiB chunk、最多 2 条并行 stream 的认证加密；实现的内存上限不得随总备份体积线性增长。
- master key 不进入备份；manifest 只记录 fingerprint。backup key 由部署侧 `BackupKeyProvider` 提供，key material 不序列化、不打印，并在释放时清零。
- 默认兼容策略严格比较 format version、application build、migration head 与 master-key fingerprint。只有独立 fixture 覆盖的显式前向路径可以放宽；不提供隐式 fallback。
- staging component 全部完成后才能 seal；seal 后重新读取并验证 component 与 envelope digest，才发布 ready BackupSet。

### Recovery safety

恢复前执行零写 preflight、创建 verified safety BackupSet、进入 maintenance fence 并排空 worker。HTTP 只创建 intent 与观察状态；实际 executor 在主 pool 外使用同一 contract 执行。跨 PostgreSQL、对象和工件使用 staging、append-only external step journal、幂等 resume 与安全备份补偿，不宣称跨介质原子事务。

任一完整性、compatibility、capacity、permission、toolchain、journal、drain 或 safety-backup 检查失败时，在覆盖目标数据前停止。补偿安全无法证明时保留 fence 并进入 `manual_recovery_required`。

### Authorization

后台注册独立 `system.backups` SettingsFeature。list/get/create/import/verify/download/delete/preflight/restore/status 使用单接口 operation；restore 额外要求 current root、recent one-shot re-auth、CSRF、精确 BackupSet 名称和 intent binding。前端不拥有兼容性、状态转移或授权真值。

## Consequences

- release image 必须提供可验证的 PostgreSQL backup/restore toolchain。
- 所有 durable/object/plugin/background write owner 必须进入有限 maintenance inventory；无法枚举即停止实现。
- 生产 BackupRepository 必须位于与业务 storage、插件安装与 MCP library 不重叠的独立故障域。
- schedule、incremental、PITR、多节点、跨 key/build migration、云 snapshot/KMS 与双人审批不在 v1；加入任一项都需重新对齐 #1667。

## Verification

全部开发与 fixture 进入冻结 assembly 后，由 #1667 的单一集中 Test Batch 在随机临时 PostgreSQL database 和显式临时对象/工件根目录执行。测试不得连接 `api/apps/api-server/.env` 的开发主库，也不得操作生产数据库或存储。
