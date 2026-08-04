---
memory_type: feedback
feedback_category: repository
topic: dev-up 自有运行输出统一使用英文 ASCII
summary: `node scripts/node/dev-up.js` 及其运行模块的自有 help、log、error 和步骤描述统一使用英文 ASCII，避免 Windows 终端编码乱码。
keywords:
  - dev-up
  - english
  - ascii
  - windows
  - terminal
match_when:
  - 新增或修改 dev-up 的 help、log、error 或步骤描述
  - 审查 dev-up 的跨平台终端兼容性
created_at: 2026-08-04 11
updated_at: 2026-08-04 11
last_verified_at: 2026-08-04 11
decision_policy: direct_reference
scope:
  - scripts/node/dev-up.js
  - scripts/node/dev-up
---

# dev-up 自有运行输出统一使用英文 ASCII

## 时间

`2026-08-04 11`

## 规则

- `node scripts/node/dev-up.js` 及 `scripts/node/dev-up/*.js` 自己生成的 help、log、error 和步骤描述使用英文 ASCII。
- Docker、pnpm、Cargo 等外部子进程原生输出保持透传，不由 dev-up 翻译或改写。
- 修改运行文案时保持 ASCII 契约测试通过。

## 原因

- Windows 终端的默认编码和字体环境可能导致中文输出乱码。
- 英文 ASCII 能为本地开发启动器提供更稳定的跨平台诊断输出。

## 适用场景

- dev-up 正常启动、停止、重启和状态输出
- CLI help 与参数错误
- Docker、端口占用、预启动和 migration drift 恢复信息
