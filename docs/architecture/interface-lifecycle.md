# 请求架构与调用生命周期

本文集中维护请求架构与调用生命周期，先看总体结构，再按目录定位主题。AGENTS 保存执行约束，skill 保存实现/验收方法，均引用本文；原 Wiki 的相关架构内容在此本地维护。

## 一次调用的总体结构

```mermaid
flowchart LR
    A["协议适配器<br/>HTTP / SSE / WebSocket / MCP / WebMCP"] --> B["统一接口层<br/>冻结计划、身份、阶段、终态"]
    B --> C["业务层<br/>typed Handler / Use Case / Domain"]
    C --> D["执行层<br/>Repository / Transaction / Runtime Port"]
    D --> E["结果与收尾"]
    E --> F["协议投影"]
```

这是逻辑调用关系，不是 Cargo 依赖图。认证由宿主 factory 在业务 Kernel Resolve 前完成，细节见[契约与身份](#contracts-and-identity)。`public / console / runtime` 等入口分区也不是上图四个责任平面；同一入口分区的请求仍经过各责任平面。

## 按问题阅读

| 顺序 | 问题 | 章节 |
| --- | --- | --- |
| 1 | 请求从哪里进入，代码放在哪里？ | [入口、装配与模块职责](#ingress-and-ownership) |
| 2 | 谁在调用，哪些身份不能变化？ | [契约、认证与冻结身份](#contracts-and-identity) |
| 3 | 执行顺序是什么，扩展能做什么？ | [执行阶段与扩展计划](#execution-and-extensions) |
| 4 | 断开、取消、提交和收尾是什么关系？ | [终态、收尾与交付](#finalization-and-delivery) |
| 5 | 怎样证明重构保持协议和业务效果？ | [等价性与验收证据](#equivalence-and-evidence) |

人通过 GUI/HTTP、AI 通过 MCP/WebMCP 调用等价业务意图时，应遵守同一核心权限、业务规则与终态语义。协议包装、调用 ID 可以不同；可关联身份和业务效果必须符合各自契约。

## 维护边界

- 本文描述架构与不变量，不将某次 CI 通过写成所有接口、所有输入已验证。
- 接口生命周期管理先提供统一契约；三级插件开放再验证真实声明、加载、组合与执行。受控 plan 测试不能替代真实插件装配，合法空计划也不是缺陷。
- 当前实现查源码，候选覆盖查对应 Issue/Assembly Receipt/测试报告；旧 receipt 保留执行版本，不改写为当前验收结果。
- 详细 crate owner 与依赖以 [crates/AGENTS.md](../../api/crates/AGENTS.md) 为准；宿主规则见 [api/AGENTS.md](../../api/AGENTS.md)。本文只解释边界，不复制完整目录表。
- 新增解释归入对应章节，保留目录导航，避免重复说明与任务日志。新增行为或改变权限/事务语义需单独定界，不能通过文档同步隐式批准。


<a id="ingress-and-ownership"></a>

## 入口、装配与模块职责

### 装配产生覆盖真值

```mermaid
flowchart TB
    R["受控 route / merge / nest"] --> M["实际 Router"]
    R --> I["实际 Endpoint Inventory"]
    G["Effective Graph + activated factories + typed handlers"] --> S["冻结 Registry snapshot"]
    I --> V["发布前交叉校验"]
    S --> V
    P["OpenAPI / descriptor / MCP methods"] --> V
    V --> C["分类完整、Binding 唯一的可发布入口"]
```

实际挂载与 Inventory 从同一构造声明产生，不能以另一份手写清单证明路由覆盖。静态协议适配器保持静态；请求期间不能临时创建 Route 或拼装另一套执行计划。

```text
ActualEndpoints = Business ⊎ ProtocolControl ⊎ OperationalControl
Business Endpoint → exactly-one Binding → exactly-one Compiled Plan
```

这些是目标集合与互斥分类约束，不表示每个 Interface 只能有一个 Binding。未分类、重复来源、未知 Binding、无实际挂载和业务/控制分类冲突应在发布前拒绝。控制入口只能进入明确的有限分类，不能把业务入口改名为 control 来绕过 Kernel。

### 四个责任平面

| 平面 | 拥有的复杂度 | 不负责 |
| --- | --- | --- |
| Protocol Adapter | 协议解析、credential 接入、结果投影 | 绕过计划直接执行业务 |
| Canonical Interface | Definition/Binding/Plan、调用阶段与 Receipt | 业务事务、凭证解析、插件加载 |
| Business | 业务授权策略、不变量、状态流转和事务意图 | 解析 Cookie/SSE/MCP transport |
| Execution | SQL、存储、编排与窄 Runtime Port 的执行 | 重新定义业务权限或调用身份 |

`api-server` 是 Composition Root：把声明与具体实现装配到稳定 typed ports。Kernel 调用这些 ports，并不因此依赖具体 service、Storage、Plugin Framework 或 Runtime Host。

```mermaid
flowchart LR
    IR["interface-runtime"] --> D["domain"]
```

此图才表示该 crate 的内部直接依赖。完整 Cargo 边只维护在 [crate 职责表](../../api/crates/AGENTS.md)；不能把业务调用箭头误读成允许新增依赖。

### 多协议与嵌套调用

HTTP/SSE/WebSocket、MCP/WebMCP 保持各自输入、错误和流式协议。WebMCP 外层管理自身调用生命周期，下游业务 Invocation 通过 lineage 关联；下游已收尾不能证明外层也已收尾。

Runtime Worker 是 Dispatch 后的执行目标；Background Worker/Schedule 是主动调用入口，两者不同。内部入口全集、System Principal、durable retry/ack 必须由其 owner 明确，不能由统一逻辑图推导为已全部接入。

### 源码入口

- [external_route_assembly.rs](../../api/apps/api-server/src/external_route_assembly.rs)：挂载与 Inventory 同源构造。
- [external_endpoint_catalog.rs](../../api/apps/api-server/src/external_endpoint_catalog.rs)：覆盖分类与发布校验。
- [WebMCP interface](../../api/apps/api-server/src/routes/webmcp/interface.rs)：外层调用接入。


<a id="contracts-and-identity"></a>

## 契约、认证与冻结身份

### 六个对象回答六个问题

| 对象 | 回答的问题 |
| --- | --- |
| Interface Definition | 做什么：typed input/output/event/error、权限和执行模式 |
| Protocol Binding | 从哪里调用：协议入口如何对应业务契约 |
| Principal | 谁在调用：可信主体与授权上下文 |
| Compiled Plan | 本次采用哪些认证、授权、准入、Hook 和 Handler |
| Attempt | 由哪个目标、运行代次执行 |
| Receipt | 实际经历哪些阶段、观察和终态 |

Definition 不拥有 HTTP Header、数据库事务或 Runtime Worker。Binding 不拥有业务规则；一次调用按明确 binding_id 和 protocol 解析唯一计划，不能任取某 Interface 的第一条 Binding。

### 认证在适配器终止凭证传播

```mermaid
flowchart LR
    R["解析 Binding / activation<br/>冻结 snapshot、建立 attempt lineage"] --> A["宿主认证 factory<br/>瞬时 credential"]
    A -->|成功| P["sealed Principal<br/>沿用 lineage 的 Envelope"]
    A -->|拒绝| E["Principal 未建立<br/>拒绝 Receipt + 适用 Completion"]
    P --> K["业务 Kernel Resolve"]
```

PublicPrincipal 不伪造 Actor；UserPrincipal/ApplicationPrincipal 内的 ActorContext 是授权真值。Application 同时携带契约要求的 application、api_key、workspace 身份。Cookie、Bearer Token、Session secret 和 API Key 原文不进入 Kernel、Handler、Receipt 或普通插件。

已解析入口的认证 attempt 在凭证验证前建立关联身份。拒绝记录包含安全的错误分类、阶段/时间和冻结身份，并发布到宿主观察渠道；不能只构造后丢弃。未认证不伪装为 Public 或空 Actor。

无法解析 Binding/activation、协议语法错误和认证 bootstrap 属于入口诊断边界，不捏造已解析业务计划。已有委托/WebSocket身份复用遵循原契约，不跳过应有委托裁决；不同消息仍有各自 Invocation identity。

### 同一 HTTP carrier 的受限变体

Compact 的 unary 执行可以投影为 SSE；有效工具回调则可能继续原 streaming resume。协议格式不能直接决定业务执行模式。

需要已认证上下文才能选择变体时，成功认证 attempt 只能在业务 Resolve 前被消费，并保持同一 snapshot、HTTP method/route、完整 authentication activation/adapter/policy、Principal profile 和 input contract。输出或执行模式可按已注册变体不同，但后续必须执行所选完整计划。

未知 Binding、其他入口或认证配置不匹配时拒绝；不重复认证、不重读当前可变 Registry、不回退为丢失 lineage 的 Envelope。认证失败仍使用入口候选计划记录拒绝。进入业务 Resolve 后不再切换 Binding/Plan。

### 两次冻结

```text
Resolve-time：Interface / Binding / Graph / Registry / Plan
Dispatch-time：Attempt / Handler / Target / Artifact / Runtime / Worker generation
```

在途调用保留旧快照，不混入新版本策略或实现。已 Dispatch 的 target/generation 不可覆盖；retry 使用新的 Attempt identity 与递增 ordinal。这是身份约束，不表示 Kernel 已实现 durable retry 调度或全历史聚合。

源码：[authentication.rs](../../api/crates/interface-runtime/src/authentication.rs)；[公共类型入口](../../api/crates/interface-runtime/src/lib.rs)。


<a id="execution-and-extensions"></a>

## 执行阶段与扩展计划

### 主路径与分支

```mermaid
flowchart TB
    R["Resolve / PrincipalEstablished"] --> Z["Core Authorization → ordered extension veto"]
    Z --> A["Core Admission → ordered extension veto"]
    A --> B["typed Before"]
    B --> D["Dispatch: 冻结 Attempt / Target"]
    D --> H["exactly-one typed Handler"]
    H -->|成功| S["After"]
    H -->|失败| F["Failure"]
    Z -->|拒绝| F
    A -->|拒绝| F
    B -->|拒绝或失败| F
    S --> C["适用 Completion"]
    F --> C
    D -->|取消或超时| C
    C --> T["Terminal Receipt"]
```

图示主要分支；取消可在其他受控等待阶段发生，按实际到达阶段执行适用收尾。宿主已经完成的凭证认证不在 Kernel 重复执行；Kernel 确认可信主体和认证配置。未解析入口与认证拒绝见[认证边界](#contracts-and-identity)。

### 阶段 owner

| 阶段 | 责任 |
| --- | --- |
| Received | 协议解析、大小限制与 correlation |
| Resolved / PrincipalEstablished | 验证绑定、冻结计划并确认可信主体 |
| Authorized | 业务 owner 判定调用资格，扩展只能进一步否决 |
| Admitted | 业务 owner 判定额度、状态、容量等准入条件 |
| Prepared | typed Before 校验与获准的有限输入修改 |
| Dispatched / Executing | 冻结目标后执行业务，事务仍由业务 owner 持有 |
| PostProcessed / Completion | 观察主结果与收尾，不改写已提交业务事实 |
| Terminal / Projected | Kernel 记录终态；适配器另行投影协议 |

Core deny 不可被 extension allow 恢复；拒绝后不运行 Handler。Definition、Decision、Hook、Handler 通过真实注册编译为可执行计划，不能用手写 fingerprint 代替绑定。

### 扩展坐标与开放边界

接口层坐标包括 definition、authentication_adapter、authorization、admission、before、handler、after、failure、completion。节点存在不等于 HostExtension、RuntimeExtension、CapabilityPlugin 都获得同样权限。

```text
合法空计划             → 正常核心生命周期
已绑定且适用的计划     → 按冻结顺序执行，或记录明确未执行原因
声明存在但实现不合法   → 在装配边界拒绝，不能静默降级为空计划
```

插件的空间坐标是目标 Interface、point/phase、scope、权限和隔离；时间坐标是 version、Graph/Registry、artifact/generation、Invocation/Attempt。每项实际开放贡献需声明 typed contract、ordering、可见事实、mutation/failure/delivery 语义和身份。

接口管理可以用受控 typed registration 验证执行契约。三级插件开放则需真实 declaration → loader/activation → graph/registry → invocation，覆盖依赖、冲突、停用、版本切换和在途隔离。native HostExtension 的 restart-scoped 管理不能被快照测试解释成 Rust 热卸载。

源码与局部规则：[interface-runtime/AGENTS.md](../../api/crates/interface-runtime/AGENTS.md)。


<a id="finalization-and-delivery"></a>

## 终态、收尾与交付

### 三种状态分别记录

```mermaid
flowchart LR
    I["Invocation terminal<br/>调用结果"]
    B["Business commit / rollback<br/>事务事实"]
    D["Delivery / ACK<br/>投影、网络、订阅者确认"]
    I -. "不能推导" .-> B
    B -. "不能推导" .-> D
```

Cancelled ≠ RolledBack；Committed 不推出 Delivered；Completion ≠ Subscriber ACK。事务 owner 保证业务变更与所需 Outbox fact 原子性，Kernel 不查数据库猜测提交，协议层不因发送失败把业务改成未发生。

### 主终态与协议投影

| 终态 | 含义 |
| --- | --- |
| Completed | 业务目标成功完成 |
| Rejected | 入口契约、认证、授权或准入拒绝；不伪造尚未建立的计划/身份 |
| Failed | 执行故障或目标失败，具体分类依 typed contract |
| Cancelled | 调用取消或 deadline 到期 |

Unary Output、Stream Events、Target Error 与 Platform Failure 先按 canonical contract 分类，再映射到 HTTP status/body、SSE event、WebSocket frame 或 MCP result/error。协议错误壳可不同，不能因此丢失原业务错误语义。

敏感认证信息按安全边界裁剪；Provider stdout/stderr/upstream error 则遵守既定透传契约，不借通用脱敏规则任意改写、截断、翻译或吞掉排障信息。

Completed 不等于 Projected；Projected 也不是客户端收到或 subscriber 已确认的证据。流式事件与唯一 terminal 分开管理。

### 有界观察与收尾

适用 After/Failure/Completion 共用独立的 **1,000 ms 总预算**。业务取消/deadline 不直接取消该预算；快速 Completion 仍有机会执行。观察器故障、panic 或超时不覆盖主结果。

每个适用 Hook 记录 identity、point、Executed/Failed/TimedOut/NotRun 和安全原因。预算耗尽后未运行的 Hook 不能记为成功。超时依赖可信 native 异步代码配合调度，不保证抢占阻塞线程或进程强杀后仍执行；可靠副作用走事务与 Outbox。

### 连接生命周期与业务取消

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

源码：[finalization.rs](../../api/crates/interface-runtime/src/finalization.rs)、[stream.rs](../../api/crates/interface-runtime/src/stream.rs)、[turn_bridge.rs](../../api/apps/api-server/src/routes/application_public_api/responses_websocket/turn_bridge.rs)。


<a id="equivalence-and-evidence"></a>

## 等价性与验收证据

### 等价性比较什么

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

### 在线时序必须可证明

```text
受控首delta barrier → 确认running → 断开/取消动作
                 → 确认连接状态 → 释放barrier → 核对业务与交付结果
```

固定 sleep 不能证明动作发生在业务终态前。并发需确认服务端连接同时存在、run/nonce隔离及无串话；同provider池原本串行时，不能把上游不重叠判为回归，也不能据连接并发声称性能等价。

错误矩阵要逐行真正请求并检查原始错误、wire terminal和持久结果。重试分别检查失败与后续成功；失败行仍保留已取得的时序、trace和持久证据，取证失败另行标注。

### 证据不能互相冒充

| 已有证据 | 不能直接推出 |
| --- | --- |
| Cargo成功退出 | 指定过滤器实际执行了测试 |
| 声明20行矩阵 | 20行在线测试已运行通过 |
| 直连mock WebSocket | 真实Gateway WebSocket通过 |
| service测试通过 | HTTP/MCP协议与认证链通过 |
| 受控typed plan通过 | 真实插件加载与组合通过 |
| 本轮专项通过 | 全仓、前端、性能或全输入空间通过 |

每条结果绑定执行SHA、命令/场景、实际计数和artifact。复用证据保留原执行SHA并说明相关源码/fixture/config身份与影响面；不改写成当前重跑。

### 维护与验证入口

实现流程见 [backend-development](../../.agents/skills/backend-development/references/interface-lifecycle.md)，验收方法见 [qa-evaluation](../../.agents/skills/qa-evaluation/references/backend/interface-lifecycle-gate.md)。本文定义证明边界，不保存不断变化的全绿计数。

已有测试入口包括 [HTTP/MCP成对fixture](../../api/apps/api-server/src/_tests/interface_lifecycle_acceptance/create_pair.rs)、[流式收尾fixture](../../api/apps/api-server/src/routes/application_public_api/responses_websocket/tests/turn_finalization.rs)、[在线生命周期](../../scripts/node/ai-gateway-concurrency/responses-websocket-acceptance/lifecycle.js)和[在线错误矩阵](../../scripts/node/ai-gateway-concurrency/responses-websocket-acceptance/error-matrix.js)。这些路径用于定位测试，不表示本次文档整理重新执行了测试。
