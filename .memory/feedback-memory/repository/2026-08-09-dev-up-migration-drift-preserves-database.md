---
memory_type: feedback
feedback_category: repository
topic: dev-up 处理 migration 元数据漂移时必须保留开发数据库
summary: 已执行 migration 的 checksum 漂移或文件缺失属于元数据漂移，应受控修复 `_sqlx_migrations` 并平滑启动，不把元数据漂移升级成整库重建。
keywords:
  - dev-up
  - migration checksum
  - worktree merge
  - preserve database
  - sqlx
  - orphan migration
match_when:
  - dev-up 遇到 previously applied but has been modified
  - dev-up 遇到 previously applied but is missing in the resolved migrations
  - worktree 分支私有 migration 污染共享开发库
  - 设计本地 migration 漂移恢复策略
created_at: 2026-08-09 14
updated_at: 2026-09-18 14
last_verified_at: 2026-09-18 14
decision_policy: direct_reference
scope:
  - scripts/node/dev-up
  - api/crates/storage-durable/postgres/migrations
---

# migration 元数据漂移优先保留数据库

## 规则

- worktree 合并导致已执行 migration 被等价改写、仅 checksum 漂移时，不建议或默认执行整库重建。
- `dev-up` 可以仅在本机开发 PostgreSQL、明确 migration 版本、数据库旧 checksum 和仓库当前 checksum 全部匹配已审核记录时，更新 `_sqlx_migrations.checksum` 后重试。
- 第二种漂移是**孤儿记录**：`_sqlx_migrations` 有已成功执行的行，但当前分支 migrations 目录没有对应文件（`previously applied but is missing in the resolved migrations`）。此时只删除该孤儿元数据行，保留业务数据与其余 schema。
- 删除孤儿行前留回滚记录（version/description/checksum/success），并保留 `tmp/` 下证据文件；分支合并回来时 `add column if not exists` 类 migration 会幂等重放。
- 未知版本、任一 checksum 不匹配、当前 migration 再次变化、远程数据库或生产环境继续失败停机；不得泛化为自动接受任意 migration 修改。
- `postgres-reset.js` 的 `ONEFLOWBASE_DEV_UP_ALLOW_DB_RESET` 是整库 DROP 的显式授权闸门，不是这类漂移的默认解法。

## 原因

用户纠正：这类故障通常来自开发 worktree 合并或分支切换，数据库业务内容并未损坏；删除整个本地数据库会把 migration 元数据问题不必要地升级成数据丢失。

## 适用场景

- `node scripts/node/dev-up.js` 的 migration drift 恢复
- SQLx `_sqlx_migrations` checksum 校验失败
- 多 worktree 开发后的 migration 合并冲突或等价重写

## 复发根因（2026-09-18 14 实证）

多个 worktree（如 `git_worktree/issue-2039`）没有自己的 `api/apps/api-server/.env` 时，dev-up 会从 `.env.example` 播种，而模板的 `API_DATABASE_URL` 默认指向主 worktree 同一个 `127.0.0.1:35432/1flowbase`；在分支 worktree 跑 dev-up 会把分支私有 migration 写进共享库。根治手段是给 worktree 配置独立 database/端口/cookie 的 `.env`。
