# Foundation Audit Cards

## Contents

- [Goal](#goal)
- [Invariants](#invariants)
- [Evidence](#evidence)
- [Legal Negatives](#legal-negatives)
- [Severity](#severity)
- [Resource Boundary](#resource-boundary)
- [Stop Conditions](#stop-conditions)

## Goal

为 AI Gateway、MCP Gateway、Application Backend、Native React / 低代码块提供有限审计不变量。统一审计只聚合 completeness 与 candidate-bound evidence，不拥有或复制各基座产品语义，不建立跨基座超级 DSL 或统一运行时。

## Invariants

共同证据模型：

```text
FoundationContract =
  Identity + StateSet + LegalTransitions + Invariants
  + ProjectionMatrix + ControlledNegatives + EvidenceBoundary
```

### AI Gateway

```text
GatewayCorrect =
  ProtocolFidelity ∧ AINativeCapability ∧ ProviderConformance
  ∧ StreamTerminalCausality ∧ ErrorFidelity
```

- terminal 为 absorbing，成功终态唯一，late event 必须拒绝。
- chunk partition 不得改变 UTF-8、segment 顺序、重复内容、terminal 或 durable answer。
- `required_capabilities ⊆ declared_capabilities`；Provider wire lowering 留在 Provider owner。
- mapper、oracle、mock、Provider capability 可多实现，但必须有 canonical event completeness 对照。

### MCP Gateway

```text
Callable = ApiKey ∧ InstanceEnabled ∧ Visible ∧ ToolEnabled
  ∧ TargetAvailable ∧ ACLAllows ∧ MappingValid ∧ DesIdCurrent
```

- `Callable ⇒ GetVisible ⇒ ListDiscoverable`，三入口不可各自漂移。
- `mcp.list -> mcp.get -> mcp.call` 是核心主链；`mcp.result` 只续取 detail。
- write succeeded 后 detail cache 不可用，不得提示重试原写操作；durable receipt 保留 outcome。

### Application Backend

```text
RuntimeAvailable = Published ∧ RegistryAvailable ∧ ScopeGrantEnabled
  ∧ ActionAllowed ∧ EffectiveScopeMatches

MetadataGeneration = PhysicalSchemaGeneration = RuntimeRegistryGeneration
```

- DTO/domain/schema/runtime 字段与 workspace/system scope 保持同一语义。
- 状态集合、合法 transition、写入口、reconcile/retry 和 Broken/Disabled 行为必须有 owner。
- mutation commit 与 registry rebuild 属 dual-write seam，失败必须 observable、retryable，不能静默 stale。
- route/docs/HTTP method/ACL action 的 CRUD 投影需要 compiled completeness 对照。

### Native React

```text
ArtifactIdentity =
  H(Source) × CompilerABI × RuntimeABI × RuntimeFingerprint × H(DependencyLock)

CommitAsyncResult = CurrentGeneration ∧ NotAborted ∧ CurrentTask ∧ CurrentEpoch
```

- standard Component、compiler/runtime ABI、dependency lock、capability、integrity 与 namespace 必须同时成立。
- Signal graph 必须 DAG、schema assignable，stale generation/epoch output 必须拒绝。
- runtime capability guard 不是安全 sandbox；JSON Schema assignability 只结算声明支持的子集。
- compiler/evaluator 相似链路用 golden IR equivalence 审计，不按文件名判定重复。

## Evidence

- AI：protocol/transport matrix、metamorphic chunk、terminal/durable parity、cursor、error fidelity、Provider paired SHA。
- MCP：list/get/call ACL 正反矩阵、mapping round-trip、stale `des_id`、oversized receipt、bundle transaction。
- Application Backend：transition、migration/reconcile replay、fault injection、schema/registry parity、CRUD/OpenAPI/ACL inventory。
- Native React：identity mutation、byte LRU、generation/epoch negative、DAG cycle/schema、dependency hash、browser/Portal fixture。

每个基座仍需读取 `../governance/foundation-contract-gates.md` 和对应 backend/frontend 专项；本卡不替代既有 fast/full receipt。

## Legal Negatives

- 不同 public protocol/provider adapter 的 mapper 职责不同，不因事件名重复直接合并。
- MCP discoverable、visible、callable 是递进子集，不要求所有函数变成同一入口，但必须满足蕴含关系。
- Application Backend adapter/DTO/mapper 若承担边界隔离，不因字段同构自动删除。
- Native compiler、source evaluator、runtime factory 的阶段责任不同，不因转换形状相似判为重复。
- 浏览器视觉、供应商私有语义、破坏性 schema 与 tool 风险等级需要人工确认，不能靠静态 fixture 完全结算。

## Severity

- `Blocking/High`：候选绑定证据证明协议、状态、ACL/scope、ABI/integrity、terminal/durable 或 generation 一致性破坏。
- `Warning`：多真值/投影漂移候选、未集中 transition、full evidence 延后、人工边界缺证据。
- `Advisory`：统一 descriptor、canonical algebra、状态表或 property fixture 的演进建议。
- `Unverified`：缺 foundation receipt、provider/browser/migration 真实证据或 candidate identity。

## Resource Boundary

- Dev/PR 复用受影响 fast pack；Provider、bundle、migration/reconcile、browser、coverage 留 nightly/manual。
- 不为审计创建跨四基座运行时、共享状态机或生成产品实现的 DSL。
- 不改变产品 API、状态语义、权限、schema、ABI、UI 或供应商 contract。
- 单次完整审计低于 1 小时；已有 receipt 足够时停止叠加重复证据。

## Stop Conditions

- 需要决定新的产品状态、API、权限、schema、ABI 或供应商语义。
- semantic duplicate 只能靠文本相似，无法限定 contract family 和 owner。
- 统一层开始复制或拥有各基座产品语义。
- 无法形成 controlled negative、candidate-bound receipt 或有限 evidence matrix。
