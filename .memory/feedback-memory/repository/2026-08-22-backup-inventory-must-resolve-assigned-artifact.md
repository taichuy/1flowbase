---
memory_type: feedback
feedback_category: repository
topic: 已分配模型提供方备份必须按可恢复制品解析
summary: 修复模型提供方备份时，不能只按 assignment 排除历史版本；还必须让已分配安装从当前节点的 ready 制品解析恢复源，不能被版本族 is_current 标记错误排除。
keywords:
  - system-backup
  - model-provider
  - assignment
  - artifact-inventory
  - is_current
match_when:
  - 修复 system_backup_source_inventory_invalid
  - 修改 plugin_management/backup_export.rs
  - 排查模型提供方版本切换、节点制品或备份恢复
created_at: 2026-08-22 19
updated_at: 2026-08-22 19
last_verified_at: 2026-08-22 19
decision_policy: direct_reference
scope:
  - api/crates/control-plane/src/plugin_management/backup_export.rs
  - api/crates/control-plane/src/plugin_management/artifact_instance.rs
---

# 已分配模型提供方备份必须按可恢复制品解析

## 时间

`2026-08-22 19`

## 规则

系统备份对模型提供方先以 workspace assignment 判定目标安装，再从当前节点选择 `artifact_status=ready` 的制品及其保留包。`is_current` 是版本族的本地选择标记，不得让它覆盖 assignment 的实际使用语义。

## 原因

先前修复只排除了未分配的历史版本；当实际被分配的版本因节点状态漂移而 `is_current=false` 时，仍会错误返回 `retained_artifact_missing`，即便保留包实际存在并可读。

## 适用场景

- 模型提供方多版本安装、版本切换和 workspace assignment 迁移。
- 备份导出、恢复前制品盘点与 retained artifact 校验。
- 节点 artifact snapshot 与版本族选择状态不一致的诊断。

## 备注

这不放宽真正的缺包保护：被 assignment 引用但当前节点没有 `ready` 制品或制品路径不可读时，仍必须 fail closed。
