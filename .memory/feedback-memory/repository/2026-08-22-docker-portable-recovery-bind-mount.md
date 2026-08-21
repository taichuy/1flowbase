---
memory_type: feedback
feedback_category: repository
topic: Docker 便携恢复的原子配置写入
summary: 容器内需要原子替换部署配置时，不能把宿主单文件直接 bind mount 为替换目标；应挂载专用可写目录并由宿主最终替换正式文件。
keywords:
  - docker
  - bind-mount
  - portable-backup
  - recovery
  - atomic-rename
match_when:
  - Docker 容器需要 rename 覆盖宿主配置文件
  - 便携备份恢复要回写部署环境变量
created_at: 2026-08-22 15
updated_at: 2026-08-22 15
last_verified_at: 2026-08-22 15
decision_policy: direct_reference
scope:
  - docker-deployment
---

# Docker 便携恢复配置写入

## 规则

恢复容器要原子替换 `deployment.env` 时，挂载一个仅用于恢复输出的目录；容器在该目录内完成 rename，成功后由宿主脚本原子替换正式 `.env`。密码 Compose env 文件独立保存，不能写入最终 `.env`。

## 原因

Docker/Linux 不允许 rename 覆盖单文件 bind mount 挂载点，会返回 `Resource busy`，使恢复在数据库写入完成后失败。

## 适用场景

Docker Shell 或 PowerShell 部署、离线恢复程序、任何需要把容器生成的配置原子持久化到宿主的流程。
