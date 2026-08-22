---
created_at: 2026-08-22 16
memory_type: project
decision_policy: verify_before_decision
scope: system backup plugin artifact inventory
---

# System backup artifact inventory repair

用户于 2026-08-22 确认采用平衡方案并授权直接修复。控制面以“恢复后必须运行，或当前节点保留 current + ready 工件”决定插件工件是否进入备份；历史安装记录继续存在于数据库备份中，但不再仅因缺少本地工件阻断备份。

`active_requested` 和 `pending_restart` 都是恢复必需状态：后者由 HostExtension 启动加载器在进程重启后激活。可重建来源只保存有效身份；不可重建来源必须有 current、ready、可读保留包。清单状态冲突以安全的 reason、installation ID 与 artifact identity 映射为 `409 system_backup_source_inventory_invalid`，仓储和 IO 基础设施故障保持服务端错误，并在日志保留 error chain。

动机是避免无关历史上传记录阻塞系统备份，同时不丢失已停用但仍保留的本地包，也不让恢复必需插件形成不完整备份。

用户随后确认：`configured_proxy` 只表示官方目录的传输路径经配置代理改写，不影响官方身份或可重建性。因此它与 `official_registry` 一样可凭 `valid` 身份重建；`configured_mirror` 与 uploaded 仍不在此豁免内。
