---
memory_type: feedback
feedback_category: repository
topic: dev-up 启动就绪必须暴露失败诊断
summary: dev-up 不得将端口可连接等同于服务启动成功；就绪失败须在终端输出可执行错误和当前服务日志尾部，并保留完整日志路径。
keywords:
  - dev-up
  - startup readiness
  - health check
  - failure log
match_when:
  - 修改 scripts/node/dev-up 的启动、重启、状态或日志行为
  - 排查本地服务端口监听但接口无响应
created_at: 2026-08-22 17
updated_at: 2026-08-22 17
last_verified_at: 2026-08-22 17
decision_policy: direct_reference
scope:
  - scripts/node/dev-up
---

# Dev-up 启动就绪必须暴露失败诊断

## 时间

`2026-08-22 17`

## 规则

端口监听只表示 TCP 可建立连接，不能作为服务可用的成功信号。启动失败或就绪探针失败时，终端必须输出失败原因、完整日志路径和有界日志尾部。

## 原因

当前 API 进程可监听 7800 却不响应 `/health`，`dev-up` 仍打印启动成功，掩盖实际故障并迫使开发者手动寻找日志。

## 适用场景

本地开发服务的启动、重启、状态检查和故障诊断输出。
