# Stable Core Vs Adapter Boundary

| 问题 | 放核心 | 放适配层 |
| --- | --- | --- |
| 是否应该执行这个业务动作 | Yes | No |
| 状态是否合法流转 | Yes | No |
| 第三方协议字段映射 | No | Yes |
| HTTP、RPC、消息、数据库细节 | No | Yes |
| 外部返回结构转换 | No | Yes |
| 业务规则判定 | Yes | No |

## Interface Plane

协议适配与业务核心之间有 Canonical Interface Plane：它执行冻结的身份、授权/准入 ports、Hook和终态契约，不拥有业务策略或事务。按改动读取[接口生命周期实现](interface-lifecycle.md)，不要把该层与Runtime执行层混为一谈。

## Working Rule

- 核心层回答“该不该做”
- 适配层回答“怎么接入、怎么转换、怎么落地”
- 外部变化先挡在适配层，不直接传进核心模型
- 能力边界用能力名命名，具体实现留在实现 crate 或 adapter；例如主存储边界和 PostgreSQL 实现不要混成一个概念
- 外部数据源插件属于协议翻译/接入适配层；平台权限、secret、job 状态和主库落盘仍归宿主核心与 durable storage 边界

## Host Extension Boundary

- Boot Core 拥有插件生命周期、deployment policy、权限、审计、主存储连接、安全策略和 `Resource Action Kernel`
- HostExtension 可以拥有 extension namespace 下的资源、migration、service、worker 和受控 route
- HostExtension 扩展 Core 业务时必须通过 manifest contribution、resource/action、hook、policy、validator、sidecar table 或 domain event
- HostExtension 不直接改 Core 真值表，不隐式包裹 service，不裸开任意 HTTP route
- RuntimeExtension 只实现 runtime slot；CapabilityPlugin 只贡献 workspace 显式选择的能力

## Infrastructure Boundary

- `storage-ephemeral`、`cache-store`、`distributed-lock`、`event-bus`、`task-queue`、`rate-limit-store` 是 host contract
- Redis、NATS、RabbitMQ 等具体实现只能作为 HostExtension provider 注册
- Core 拥有 session、lease、cache namespace、失效规则、task claim、event delivery 和 rate limit window 的语义
- RuntimeExtension 和 CapabilityPlugin 不能直接持有基础设施连接
- `task-queue` 默认按 at-least-once、idempotency key、visibility timeout 设计；domain event 先进入 durable outbox

## Smell Check

如果你一改协议字段名就要改核心业务规则，说明边界已经脏了。
