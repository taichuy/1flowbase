---
memory_type: project
topic: 激活角色切换与角色后台设置菜单顺序
summary: 用户确认 Web 会话只按单一激活角色授权，登录不展示角色选择而按稳定角色顺序激活首项；角色权限页支持拖拽维护 workspace 级后台设置顺序，catalog 与导航共享；全新 workspace 使用已确认的产品默认菜单顺序，未列出的注册项追加末尾。Single Issue #1613 是活动真值。
keywords:
  - active-role
  - session
  - single-role-authorization
  - settings-order
  - console-policy
  - issue-1613
match_when:
  - 实现或调整账户菜单角色切换
  - 调整 Web 会话角色授权语义
  - 调整角色后台设置列表或后台导航顺序
created_at: 2026-08-08 10
updated_at: 2026-08-08 11
last_verified_at: 2026-08-08 11
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1613
  - api/crates/domain/src/auth
  - api/crates/control-plane/src/auth
  - api/apps/api-server/src/routes/identity
  - web/app/src/app-shell
  - web/app/src/features/settings
---

# 激活角色切换与角色后台设置菜单顺序

## 谁在做什么

- 用户已确认把“激活角色”和“角色后台设置菜单顺序”合并为新的 Single Issue #1613，当前为 `grade:g4 / phase:ready`。
- Web session 增加单一激活角色；用户在账户菜单查看已绑定角色、识别当前角色并自行切换。
- 角色权限页的后台设置列表支持拖拽，顺序作为 workspace durable 配置，由 console-policy catalog 与后台导航共同消费。

## 为什么这样做

- 现有 `effective_display_role` 只是展示角色，授权仍聚合用户全部角色；这会让界面显示的当前角色与真实权限不一致。
- 多个角色各自保存后台菜单顺序无法得到唯一合并结果，因此排序 owner 是 workspace，而不是单个角色。

## 为什么要做

- 让用户清楚当前正在以哪个角色工作，并确保可见身份与真实权限一致。
- 让角色管理员整理后的后台设置顺序成为后端统一输出，而不是角色页和主导航各自排序。

## 已确认决策

- 登录不出现默认角色选择步骤；后端按稳定可用角色顺序自动激活首项，用户登录后可自行切换。
- 切换到 Manager 后，当前会话只拥有 Manager 权限，不再保留该用户其他角色权限。
- 激活角色属于 ephemeral session，不写入 `users.default_display_role`，不改变角色绑定。
- 后台设置顺序按 workspace 持久化，不按角色持久化。
- console-policy 跨请求缓存优化不包含在 #1613 中。
- 全新 workspace 的产品默认后台菜单顺序依次为：扩展中心、模型供应商、应用管理、MCP 管理、用户管理、权限管理、API 文档、API key 认证、数据源、认证中心、文件管理、系统运行、内存观察、基础设施、多语言。
- 用户明确列出的设置项保持相对顺序；当前或未来注册表中未列出的设置项确定性追加到末尾。
- 角色权限只过滤不可见菜单，不改变剩余菜单的相对顺序；workspace 已保存的拖拽顺序继续优先于产品默认顺序。

## 截止日期

- 无固定截止日期；以 #1613 的 AC-001～AC-010、集中 QA 与用户验收为关闭条件。
