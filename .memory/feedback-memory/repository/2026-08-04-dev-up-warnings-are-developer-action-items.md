---
memory_type: feedback
feedback_category: repository
topic: dev-up 启动 warning 必须作为开发者待处理证据保留并治理
summary: 用户要求处理 dev-up warning 时只修复现有 warning 根因；既有脚本启动输出全部保留，不顺手降噪或调整文案。
keywords:
  - dev-up
  - warning
  - developer-output
  - rust
  - startup-noise
match_when:
  - 诊断或调整 dev-up 开发启动输出
  - 决定成功构建中的 warning 应显示、隐藏还是治理
  - 用户要求收敛开发者终端噪音
created_at: 2026-08-04 10
updated_at: 2026-08-04 11
last_verified_at: 2026-08-04 10
decision_policy: direct_reference
scope:
  - scripts/node/dev-up
  - api
---

# dev-up warning 是开发者待处理证据

## 时间

`2026-08-04 10`

## 规则

- `dev-up` 成功启动时出现的编译 warning 是给开发者看的待处理证据，不得为了终端整洁整体静默。
- 用户只要求处理 warning 时，只修复现有 warning 根因；`Compiling`、`Finished`、`Running`、密码重置和服务状态等既有输出继续保留。
- 若 warning 暂不阻断当前工作，也应保留可见性或明确落盘位置和后续治理入口。

## 原因

- 隐藏成功构建输出会同时隐藏真实的废弃接口和未使用代码问题，让技术债失去反馈入口。
- 开发启动器的职责是提高信噪比，不是把黄色诊断伪装成无问题。

## 适用场景

- Rust `cargo run` 作为开发态预启动步骤
- dev-up 日志格式和 quiet/verbose 策略
- 编译 warning 的分类、修复与治理

## 备注

该规则不把所有 warning 自动升级为阻断门禁；它限定的是不能通过静默来冒充已治理。
