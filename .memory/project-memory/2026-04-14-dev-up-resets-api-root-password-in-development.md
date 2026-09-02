---
memory_type: project
topic: dev-up root 密码重置将从 Rust 全局编译链改为脚本
summary: `2026-09-03 07` 已用通用 Node.js 数据库账号密码重置脚本取代 Rust `reset_root_password`；`dev-up` 保留开发库 root 凭据同步、migration drift 检测与空库 API bootstrap 语义，不再为密码重置触发 Cargo。
keywords:
  - dev-up
  - api-server
  - root password
  - development
  - auth
match_when:
  - 需要排查本地 `root / change-me` 登录失败
  - 需要确认 dev-up 是否会自动同步开发态 root 密码
  - 需要判断持久化开发库的 root 密码为何会与 `.env` 漂移
created_at: 2026-04-14 09
updated_at: 2026-09-03 07
last_verified_at: 2026-09-03 07
decision_policy: verify_before_decision
scope:
  - scripts/node/dev-up/env.js
  - scripts/node/dev-up/_tests/prestart.test.js
  - scripts/node/reset-account-password.js
  - scripts/node/reset-account-password/
---

# dev-up 会在开发态启动前重置 api root 密码

## 2026-08-23 方向更新

- 用户确认 `reset_root_password` 应改为脚本，不再依赖 `api-server` library crate 与其全局 Rust 依赖图。
- 这一方向取代“继续复用当前 Rust `reset_root_password` 工具”的旧实现决策；已于 2026-09-03 实现并验收。
- 实现验收边界：保留开发环境 root 密码同步效果，不运行 Cargo，不把生产环境纳入自动重置，并避免出现与后端密码哈希 / bootstrap 语义漂移的第二套规则。

## 2026-09-03 现状复核

- 用户再次确认目标是“通用 JS 数据库账号密码重置脚本，读取后端配置并直接更新数据库”，不是优化 Rust `reset_root_password` 的编译边界。
- 原 `dev-up` 调用 Rust binary 会在密码重置阶段编译完整 `api-server` library 约 143 秒；实现后真实 Node.js 重置耗时 131 ms，一次 backend-only `dev-up` 重启总耗时 4.833 s。
- 实现时通用脚本只对已存在账号执行密码哈希更新；空库 / 账号不存在时由 API 启动 bootstrap 作为唯一语义 owner，不在 JS 重复 bootstrap 逻辑。

## 时间

`2026-04-14 09`

## 谁在做什么

- 用户反馈本地使用 `root / change-me` 登录控制台持续返回 `not_authenticated`。
- AI 排查后把修复落在标准本地启动入口 `node scripts/node/dev-up.js`，而不是改动生产态 bootstrap 语义。

## 为什么这样做

- `api-server` 的 bootstrap 只负责“首次补种 root”，不会覆盖已存在 root 用户的密码。
- 持久化开发数据库一旦有人改过 root 密码，后续即使 `.env` 仍是 `BOOTSTRAP_ROOT_PASSWORD=change-me`，登录也会持续失败。
- 生产环境不应在每次启动时自动重置 root 密码，因此不能直接把这个行为塞进通用启动路径。

## 为什么要做

- 本地开发的标准入口已经固定为 `dev-up`，开发态需要可预测、开箱即用的 root 登录体验。
- 把 root 密码同步限制在 `API_ENV` 非 production 的 dev-up prestart 阶段，可以修复本地体验，又不扩大生产风险。

## 截止日期

- 无

## 决策背后动机

- 保持生产启动语义稳定：生产仍只依赖显式环境配置，不自动改 root 密码。
- 让开发态 root 凭据回到 `.env` 单一真值源，避免 UI 登录问题反复落到数据库历史状态上。
- 原“继续复用 `api-server` Rust 工具”决策已被 2026-08-23 确认的通用 JS 脚本方向取代。
