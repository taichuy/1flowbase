---
memory_type: feedback
feedback_category: repository
topic: 只有 root 是受保护角色
summary: workspace 初始化生成的 admin、member 只是示例默认数据，必须允许修改和删除；只有 root 是受保护角色，is_builtin 不得对其他角色产生不可编辑或不可删除语义。
keywords:
  - role
  - admin
  - builtin
  - editable
  - console-policy
  - authorization
match_when:
  - 设计或诊断角色权限配置、默认角色、内建角色保护
  - 修改 role console policy、data policy、permission 或 frontstage route 写入口
created_at: 2026-08-09 10
updated_at: 2026-08-09 10
last_verified_at: 2026-08-09 10
decision_policy: direct_reference
scope:
  - api/crates/access-control
  - api/crates/control-plane/src/role
  - api/crates/storage-durable/postgres/src/role_repository
  - web/app/src/features/settings
---

# 只有 Root 是受保护角色

## 时间

`2026-08-09 10`

## 规则

`root` 操作者必须能够修改和删除 workspace 初始化生成的 `admin`、`member` 等示例默认角色。只有 `root` 角色本身受保护；`is_builtin=true` 不得让其他角色不可编辑或不可删除。

## 原因

操作者授权资格与目标角色保护是两个独立判定。`admin`、`member` 是初始化示例，不是平台不变量；把 `is_builtin` 当作保护条件会让 UI 已开放的编辑和删除动作被后端拒绝，并使用户无法替换默认角色体系。

## 适用场景

角色基本信息、普通权限、后台权限、数据策略和动态路由等写入口不得因 `is_builtin` 拒绝；删除入口也只能保护 `root`，同时可以继续执行默认成员角色接替、成员解绑等业务完整性约束。初始化逻辑不得在用户删除示例角色后于重启时将其复活。
