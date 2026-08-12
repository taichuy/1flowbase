---
memory_type: project
topic: PostgreSQL 备份客户端采用本地尽力供应与生产构建期强制打包
project_memory_state: implemented
summary: 用户确认并已落地 PostgreSQL backup toolchain 边界：dev-up 按显式路径、受控缓存、系统 PATH、固定 artifact 顺序解析，失败只禁用备份；API 与 system_recovery 统一消费显式路径；生产镜像构建期安装 PostgreSQL 18 客户端，运行期不下载。
keywords:
  - PostgreSQL
  - backup
  - pg_dump
  - pg_restore
  - dev-up
  - toolchain
  - Docker
match_when:
  - 继续调整系统备份还原工具供应
  - 判断 API 启动是否应依赖 PostgreSQL 客户端
  - 修改 dev-up 的 PostgreSQL toolchain 下载或缓存
  - 修改生产 API 镜像中的 PostgreSQL 客户端
created_at: 2026-08-12 16
updated_at: 2026-08-12 16
last_verified_at: 2026-08-12 16
decision_policy: verify_before_decision
scope:
  - scripts/node/postgres-toolchain
  - scripts/node/dev-up
  - api/apps/api-server/src/system_backup
  - api/apps/api-server/src/bin/system_recovery.rs
  - docker/api-server.Dockerfile
  - .github/workflows/container-images.yml
---

# PostgreSQL backup toolchain 供应边界

## 谁在做什么

1flowbase 已由开发工具链为本地源码开发尽力准备 `pg_dump` / `pg_restore`，由 API Server 与 `system_recovery` 统一解析工具路径，并由生产镜像构建流程强制安装 PostgreSQL 18 客户端。

## 为什么这样做

系统备份依赖 PostgreSQL 原生 dump/restore contract，但该可选能力不能成为登录、公开 API 或整个 API Server 的启动前置条件。本地环境需要跨平台且可复现，生产环境则需要不可变、离线可启动的镜像供应。

## 决策

- 本地解析顺序固定为：显式配置 → 已验证项目缓存 → 兼容的系统 PATH → `lock.json` 固定 artifact。
- 下载失败、校验失败或平台不支持时只输出 warning；API 正常启动，备份接口返回 `503 system_backup_unavailable`。
- API Server 不负责联网下载；下载仅属于 `scripts/node/postgres-toolchain` 与 `dev-up`。
- `system_recovery` 与 API 使用同一对 `API_POSTGRES_PG_DUMP_PATH` / `API_POSTGRES_PG_RESTORE_PATH`。
- 生产镜像构建期安装 `postgresql-client-18` 并验证真实 dump/list/restore fixture；生产运行期不联网下载。
- 不引入 HostExtension，也不建立通用工具管理抽象。

## 状态与截止日期

该方向已于 2026-08-12 合入本地 `dev`，等待用户人工验收；没有额外截止日期。
