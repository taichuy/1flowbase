---
name: backend-development
description: 实现或审查 1flowbase api/ 的接口、业务、状态与模块边界。先理解后端架构并定位 AGENTS / 架构章节；需求决策用 problem-framing，正式验收用 qa-evaluation。
---

# Backend Development

## Outcome and Entry

把已确认目标落到正确的后端 owner，保持 contract、状态和调用边界。先读 [api/AGENTS.md](../../../api/AGENTS.md)；改 crate 前读 [crates/AGENTS.md](../../../api/crates/AGENTS.md)，再读改动目录的局部规则。复用用户已确认的目标、授权与 AC，不重复审批。

行为验证使用 `test-driven-development`；只有业务语义、权限、数据影响或范围尚未确定时回到 `problem-framing`，不把局部实现判断变成新的产品决策。

## Backend Architecture Map

先理解 [请求架构与调用生命周期](../../../docs/architecture/interface-lifecycle.md) 的总体结构与维护边界，再按任务读主题，无需先遍历全项目架构。

```text
api-server（唯一进程内 composition root）
  Protocol Adapter → Canonical Interface / 冻结计划
                   → typed Handler → Business / Execution
                   → 结果与收尾 → 协议投影
```

- Protocol Adapter 负责协议解析、认证接入和响应投影；业务入口经注册 Binding / Plan 进入统一调用内核。
- Canonical Interface 拥有调用身份、阶段与终态；业务 owner 拥有规则和事务，执行与存储模块承接稳定 ports。
- 调用终态、业务 commit/rollback、协议 delivery/ack 各有 owner，不能互相推断；关闭连接不自动等于取消业务。
- 这是调用关系，不是 Cargo 依赖图。crate owner 与允许依赖只维护在 `api/crates/AGENTS.md`；实际 mount 与 Catalog、声明与编译快照的关系由架构文档解释。
- HostExtension 扩展宿主 contract；RuntimeExtension 实现 runtime slot；CapabilityPlugin 贡献用户选择的能力。具体生命周期和允许写入口按相关局部规则取证，不把三者混为同一插件类型。

## Truth and Task Routing

架构文档定义关系，AGENTS 定义执行约束，源码与运行态揭示实际行为。三者不一致时定位偏离；不自动扩大授权去改架构或业务语义。

| 当前任务 | 读取与定位 |
| --- | --- |
| 接口装配、认证、Binding、Kernel、stream / cancel / deadline | [interface-lifecycle](references/interface-lifecycle.md) → 架构对应主题 |
| API 输入输出、错误模型 | [api-design](references/api-design.md) → route / DTO / typed Handler |
| 状态转换、事务、幂等 | [state-and-consistency](references/state-and-consistency.md) → 状态写 owner |
| 核心与适配器、外部依赖、HostExtension | [boundary-design](references/boundary-design.md) → 局部 AGENTS / ports |
| 新资源或实现落点 | [implementation-rules](references/implementation-rules.md) |
| Rust 类型、async、锁与实现自查 | [rust-backend-practices](references/rust-backend-practices.md) 的相关章节及 completion self-check |
| 内置模型、metadata overlay、runtime read model | [builtin-data-model-contract](references/builtin-data-model-contract.md) |
| Settings API、注册、角色 operation 授权 | [console-settings-registration](references/console-settings-registration.md) |
| Agent Flow 节点输入、debug artifact、运行日志 | [agentflow-runtime-node-payload](references/agentflow-runtime-node-payload.md) |
| 新抽象、公共参数、重复防御、转发层 | [design-rules](../_shared/design-rules.md)；具体坏味道查 [anti-patterns](references/anti-patterns.md) |
| 判断规则例外 | [examples](references/examples.md) |

## Implementation Decisions

- 状态变更经唯一明确 owner；核心规则不依赖外部协议格式、存储细节或临时 UI 形态。
- 公共接口表达具体对象和动作，Rust 类型表达不变量，错误显式传播；不为拆分而增加无职责包装层。
- 前端缺业务字段、排序 / 筛选 / 聚合结果时补职责单一的后端 DTO / API / 查询；字段沿用 DTO / 领域原名。
- 系统内置数据与用户 metadata、宿主与插件写集等专项边界，按上表加载对应真值；不因任何后端改动加载所有专项。
- 新增或改变目标、source of truth、权限、历史数据处理或对外 contract 时回到需求对齐；已批准范围内的装配、fixture 与局部实现修正继续。

## Evidence and Exit

完成开发后用 `qa-evaluation` 对照架构与行为验收。交付保留 route / service / domain / adapter 等改动 owner、关键决策与证据指针；已有 AC 标明已覆盖、未覆盖及延后项。

执行成本、Cargo 并发、证据复用与资源失败转向沿用 `test-driven-development`。当前行为与直接风险证据充分即停止；编译或源码门禁不代替行为测试，未运行的重型验证不冒充通过。
