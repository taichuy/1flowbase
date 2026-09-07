# 终态、收尾与交付

[总览](README.md) · 上一篇：[执行与扩展](03-execution-and-extensions.md) · 下一篇：[等价验收](05-equivalence-and-evidence.md)

## 三种状态分别记录

```mermaid
flowchart LR
    I["Invocation terminal<br/>调用结果"]
    B["Business commit / rollback<br/>事务事实"]
    D["Delivery / ACK<br/>投影、网络、订阅者确认"]
    I -. "不能推导" .-> B<br/>    B -. "不能推导" .-> D
```

Cancelled ≠ RolledBack；Committed 不推出 Delivered；Completion ≠ Subscriber ACK。事务 owner 保证业务变更与所需 Outbox fact 原子性，Kernel 不查数据库猜测提交，协议层不因发送失败把业务改成未发生。

## 主终态与协议投影

| 终态 | 含义 |
| --- | --- |
| Completed | 业务目标成功完成 |
| Rejected | 入口契约、认证、授权或准入拒绝；不伪造尚未建立的计划/身份 |
| Failed | 执行故障或目标失败，具体分类依 typed contract |
| Cancelled | 调用取消或 deadline 到期 |

Unary Output、Stream Events、Target Error 与 Platform Failure 先按 canonical contract 分类，再映射到 HTTP status/body、SSE event、WebSocket frame 或 MCP result/error。协议错误壳可不同，不能因此丢失原业务错误语义。

敏感认证信息按安全边界裁剪；Provider stdout/stderr/upstream error 则遵守既定透传契约，不借通用脱敏规则任意改写、截断、翻译或吞掉排障信息。

Completed 不等于 Projected；Projected 也不是客户端收到或 subscriber 已确认的证据。流式事件与唯一 terminal 分开管理。

## 有界观察与收尾

适用 After/Failure/Completion 共用独立的 **1,000 ms 总预算**。业务取消/deadline 不直接取消该预算；快速 Completion 仍有机会执行。观察器故障、panic 或超时不覆盖主结果。

每个适用 Hook 记录 identity、point、Executed/Failed/TimedOut/NotRun 和安全原因。预算耗尽后未运行的 Hook 不能记为成功。超时依赖可信 native 异步代码配合调度，不保证抢占阻塞线程或进程强杀后仍执行；可靠副作用走事务与 Outbox。

## 连接生命周期与业务取消

```text
连接关闭 / writer失败 / 桥接任务abort
  → 停止该连接的交付
  → 保留独立completion owner完成收尾

显式业务取消接口
  → 请求业务状态转移
  → 按既有协议投影取消终态
```

`InterfaceStreamCompletion::complete()` 必须有明确持有并等待的 owner；drop 不代表收尾成功。WebSocket 桥接把 completion 交给独立任务，使传输任务退出不丢掉内核收尾。后台任务存在也不表示结果已送达客户端。

关闭连接不自动调用业务取消；业务取消也不自动保证远端 provider 停止计算。Responses WebSocket 当前不因本说明新增 `response.cancel` 操作，显式取消使用既有 Native API 契约。

Outbox lease/retry/去重/每订阅者 ACK 与 PluginData 局部幂等归各自 owner；调用生命周期不自动提供全接口幂等或端到端 exactly-once。

源码：[finalization.rs](../../../api/crates/interface-runtime/src/finalization.rs)、[stream.rs](../../../api/crates/interface-runtime/src/stream.rs)、[turn_bridge.rs](../../../api/apps/api-server/src/routes/application_public_api/responses_websocket/turn_bridge.rs)。
