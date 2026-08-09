---
memory_type: feedback
feedback_category: repository
topic: dev-up 处理已知等价 migration 漂移时必须保留开发数据库
summary: worktree 合并导致已执行 migration 的 checksum 发生已确认等价漂移时，dev-up 应受控修复 migration 元数据并平滑启动，不把元数据漂移升级成整库重建。
keywords:
  - dev-up
  - migration checksum
  - worktree merge
  - preserve database
  - sqlx
match_when:
  - dev-up 遇到 previously applied but has been modified
  - worktree 合并改写了已执行 migration
  - 设计本地 migration 漂移恢复策略
created_at: 2026-08-09 14
updated_at: 2026-08-09 14
last_verified_at: 2026-08-09 14
decision_policy: direct_reference
scope:
  - scripts/node/dev-up
  - api/crates/storage-durable/postgres/migrations
---

# 已知等价 migration 漂移优先保留数据库

## 规则

- worktree 合并导致已执行 migration 被等价改写、仅 checksum 漂移时，不建议或默认执行整库重建。
- `dev-up` 可以仅在本机开发 PostgreSQL、明确 migration 版本、数据库旧 checksum 和仓库当前 checksum 全部匹配已审核记录时，更新 `_sqlx_migrations.checksum` 后重试。
- 未知版本、任一 checksum 不匹配、当前 migration 再次变化、远程数据库或生产环境继续失败停机；不得泛化为自动接受任意 migration 修改。

## 原因

用户纠正：这类故障通常来自开发 worktree 合并，数据库业务内容并未损坏；删除整个本地数据库会把 migration 元数据问题不必要地升级成数据丢失。

## 适用场景

- `node scripts/node/dev-up.js` 的 migration drift 恢复
- SQLx `_sqlx_migrations` checksum 校验失败
- 多 worktree 开发后的 migration 合并冲突或等价重写
