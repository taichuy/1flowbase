---
memory_type: project
topic: Admin 初始权限与自动授予新增权限
summary: 用户确认只修复最新版本的新初始化，不处理历史权限数据；同时保证 Admin 自动授予新权限机制。
created_at: 2026-10-07 16
updated_at: 2026-10-07 16
decision_policy: verify_before_decision
status: active
---

Root 按用户 2026-10-07 的确认修复新部署及新工作区的 Admin 初始授权，同时验证自动授予新增权限、取消勾选以及手动撤权的保留。用户明确排除历史数据修复，因为此次目标是最新版本开箱即用。无固定截止日期。

原 Compose 试验实例保留原数据和原镜像，不据此判断新初始化的授权结果。源码修复不等于镜像已发布或该实例已部署更新。
