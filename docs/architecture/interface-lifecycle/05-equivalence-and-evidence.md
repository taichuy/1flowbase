# 等价性与验收证据

[总览](README.md) · 上一篇：[终态与交付](04-finalization-and-delivery.md)

## 等价性比较什么

固定业务意图、身份权限、初始数据和配置，比较重构前后各协议的输入接受/拒绝、返回和实际副作用。比较 HTTP 与 MCP 时分别锁定协议壳，再比较业务语义，不要求包装或随机 ID 一模一样。

| 维度 | 有效证据 |
| --- | --- |
| 入口 | 真实 Router/MCP dispatch；实际 mount、Catalog 与 Binding 对应 |
| 输入与权限 | 正常、缺字段、非法值、未授权及 scope 越权的正反例 |
| 输出 | 字段、数组顺序、status、完整错误壳和 message、流式事件顺序与终态 |
| ID | 先逐端核对响应 ID 与本端落盘对象，再规范化跨端随机身份 |
| 副作用 | 目标写入、完整 grants、审计、非目标行不变；真实 commit/rollback |
| 生命周期 | frozen snapshot/attempt、取消/超时、observer故障、独立收尾与交付失败 |

两个新入口彼此一致不能单独证明兼容旧版；必须保留旧契约、源码身份或基线 fixture。不要排序字段数组或删除错误细节来掩盖差异。

## 在线时序必须可证明

```text
受控首delta barrier → 确认running → 断开/取消动作
                 → 确认连接状态 → 释放barrier → 核对业务与交付结果
```

固定 sleep 不能证明动作发生在业务终态前。并发需确认服务端连接同时存在、run/nonce隔离及无串话；同provider池原本串行时，不能把上游不重叠判为回归，也不能据连接并发声称性能等价。

错误矩阵要逐行真正请求并检查原始错误、wire terminal和持久结果。重试分别检查失败与后续成功；失败行仍保留已取得的时序、trace和持久证据，取证失败另行标注。

## 证据不能互相冒充

| 已有证据 | 不能直接推出 |
| --- | --- |
| Cargo成功退出 | 指定过滤器实际执行了测试 |
| 声明20行矩阵 | 20行在线测试已运行通过 |
| 直连mock WebSocket | 真实Gateway WebSocket通过 |
| service测试通过 | HTTP/MCP协议与认证链通过 |
| 受控typed plan通过 | 真实插件加载与组合通过 |
| 本轮专项通过 | 全仓、前端、性能或全输入空间通过 |

每条结果绑定执行SHA、命令/场景、实际计数和artifact。复用证据保留原执行SHA并说明相关源码/fixture/config身份与影响面；不改写成当前重跑。

## 维护与验证入口

实现流程见 [backend-development](../../../.agents/skills/backend-development/references/interface-lifecycle.md)，验收方法见 [qa-evaluation](../../../.agents/skills/qa-evaluation/references/backend/interface-lifecycle-gate.md)。本文定义证明边界，不保存不断变化的全绿计数。

已有测试入口包括 [HTTP/MCP成对fixture](../../../api/apps/api-server/src/_tests/interface_lifecycle_acceptance/create_pair.rs)、[流式收尾fixture](../../../api/apps/api-server/src/routes/application_public_api/responses_websocket/tests/turn_finalization.rs)、[在线生命周期](../../../scripts/node/ai-gateway-concurrency/responses-websocket-acceptance/lifecycle.js)和[在线错误矩阵](../../../scripts/node/ai-gateway-concurrency/responses-websocket-acceptance/error-matrix.js)。这些路径用于定位测试，不表示本次文档整理重新执行了测试。
