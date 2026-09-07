# 请求架构与调用生命周期

本目录是接口生命周期架构的仓库内维护入口。每篇回答一个问题，先看总览，再按任务阅读；AGENTS 保存执行约束，skill 保存实现/验收方法，均引用这里。原 Wiki 的相关架构内容已整理到本目录，后续维护以本地系列为入口。

## 一次调用的总体结构

```mermaid
flowchart LR
    A["协议适配器<br/>HTTP / SSE / WebSocket / MCP / WebMCP"] --> B["统一接口层<br/>冻结计划、身份、阶段、终态"]
    B --> C["业务层<br/>typed Handler / Use Case / Domain"]
    C --> D["执行层<br/>Repository / Transaction / Runtime Port"]
    D --> E["结果与收尾"]<br/>    E --> F["协议投影"]
```

这是逻辑调用关系，不是 Cargo 依赖图。认证由宿主 factory 在业务 Kernel Resolve 前完成，细节见第二篇。`public / console / runtime` 等入口分区也不是上图四个责任平面；同一入口分区的请求仍经过各责任平面。

## 按问题阅读

| 顺序 | 问题 | 文章 |
| --- | --- | --- |
| 1 | 请求从哪里进入，代码放在哪里？ | [入口、装配与模块职责](01-ingress-and-ownership.md) |
| 2 | 谁在调用，哪些身份不能变化？ | [契约、认证与冻结身份](02-contracts-and-identity.md) |
| 3 | 执行顺序是什么，扩展能做什么？ | [执行阶段与扩展计划](03-execution-and-extensions.md) |
| 4 | 断开、取消、提交和收尾是什么关系？ | [终态、收尾与交付](04-finalization-and-delivery.md) |
| 5 | 怎样证明重构保持协议和业务效果？ | [等价性与验收证据](05-equivalence-and-evidence.md) |

人通过 GUI/HTTP、AI 通过 MCP/WebMCP 调用等价业务意图时，应遵守同一核心权限、业务规则与终态语义。协议包装、调用 ID 可以不同；可关联身份和业务效果必须符合各自契约。

## 维护边界

- 本系列描述架构与不变量，不将某次 CI 通过写成所有接口、所有输入已验证。
- 接口生命周期管理先提供统一契约；三级插件开放再验证真实声明、加载、组合与执行。受控 plan 测试不能替代真实插件装配，合法空计划也不是缺陷。
- 当前实现查源码，候选覆盖查对应 Issue/Assembly Receipt/测试报告；旧 receipt 保留执行版本，不改写为当前验收结果。
- 详细 crate owner 与依赖以 [crates/AGENTS.md](../../../api/crates/AGENTS.md) 为准；宿主规则见 [api/AGENTS.md](../../../api/AGENTS.md)。本系列只解释边界，不复制完整目录表。
- 新增解释优先归入对应主题；主入口保留阅读导航，不重新累积成长篇。新增行为或改变权限/事务语义需单独定界，不能通过文档同步隐式批准。
