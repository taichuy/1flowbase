---
name: test-driven-development
description: Choose risk-proportionate tests and verification evidence for 1flowbase behavior changes. Define observable expectations before implementation, reuse failure evidence, and control execution cost; use qa-evaluation for final acceptance.
---

# Test Strategy and Evidence

## Outcome

用足以识别错误行为的证据支持开发，在风险、实际运行成本与上下文消耗之间选择验证方式。保留 `test-driven-development` 名称作为已有调用入口；先红后绿是可选策略，不由 Single Issue / Issue Tree 形态决定。

## Entry and Scope

- 承接已确认目标、范围与验收预期；用户明确要求直接实现时沿用该授权，不重复建立审批。只有业务语义或范围仍有关键歧义时回到 `problem-framing`。
- 实现前明确可观察结果及预期来源。已有 AC 编号时映射到测试或证据；简单任务无需另建矩阵。
- 纯文案、样式 token、机械修改、生成代码或无法自动化的场景，可复用现有检查或人工取证；说明未新增测试的原因与替代证据。
- 产品设计与实现规则由对应 frontend / backend skill 负责，本 skill 不重新审批方案或规定 agent 编排。

## Choose Evidence by Risk

| 场景 | 优先策略 | 证据边界 |
| --- | --- | --- |
| 缺陷、回归 | 复用真实复现；缺少时先取得最小失败证据，再修复 | 失败须来自目标行为，编译、认证或环境故障不算业务红灯 |
| 明确的新功能 | 先固定预期，允许测试与实现同批编写，完成后定向运行 | 关键断言应能拒绝代表性错误实现，避免测试照抄实现 |
| 权限、状态转换、数据完整性等高风险变化 | 用正反例、领域不变量或适当的属性测试锁定边界 | 需要真实失败证据或受控反例证明关键测试的识错能力 |
| 重构 | 优先复用既有行为测试，只补受影响边界的缺口 | 无需为了红灯故意破坏已经成立的行为 |
| 源码归属、禁止构造点等结构规则 | 优先使用已有轻量扫描器及其正反例 | 源码扫描不能代替运行行为测试 |

已有日志、失败测试或确定性扫描结果可承接相应验收点，不为满足顺序重新制造红灯。测试入口暂不可用时保留缺口并继续不依赖它的开发；关键行为缺少验证时不能宣称验收通过。

## Development and Acceptance

- 同一行为族批量实现与验证，不按每个字段或断言建立微循环。
- 默认完成本次开发后集中 QA；开发中允许能消除具体不确定性的廉价语法、类型或定向反馈，不把这些检查升级成逐段验收。
- Issue Tree 的 fixture、assembly 和集中 QA 边界遵循 [long-running-work.md](../problem-framing/references/long-running-work.md)。已有证据可复用，最终由冻结 assembly 的集中 QA 结算 AC，不逐 Packet 启动 QA。
- 只补当前变更与直接风险需要的回归；既有测试债不自动纳入修复。

## Execution Cost and Stop Conditions

以下规则同时供开发验证与 QA 使用：

- 选择入口时看实际编译、链接、服务依赖与缓存；只指定一个测试名不代表构建变小。源码扫描无需链接 Rust 服务；`cargo check --tests` 只证明测试可编译。
- 优先已有可执行入口。重型验证按 [gate-lanes.md](../qa-evaluation/references/governance/gate-lanes.md) 的资源边界安排，不为局部验收临时重构测试架构，也不按命令数量判断成本。
- 预计昂贵的命令启动前明确本次要取得的证据、可承受资源与转向条件，复用已知环境记录即可，不为估算成本另开大规模调查。
- 资源失败或运行成本明显超出预期时，先区分产品、fixture、工具和资源问题。停止当前任务拥有且已无法合理完成的验证进程；只有构建范围、资源条件或入口实质变化才重试。换测试名或 feature 本身不是成本下降的证据。
- 切换到能回答同一问题的低成本入口；没有等价入口时标明未验证及适合执行的 CI / 专门环境，继续独立开发。不得把计划在 CI 执行写成 CI 已通过。
- 同一 worktree 同时只运行一条 Cargo 命令。沿用仓库资源配置；从仓库根使用现有包装入口，或显式指定 `api/Cargo.toml` 并使用 `scripts/node/testing/verify-runtime.js cargo-jobs` 解析并行度。先确认辅助命令成功，避免路径错误后仍启动昂贵构建。

## Evidence Reuse and Context

- 在已有交付记录或 artifact 中保留命令、代码状态（SHA 或 diff 标识）、覆盖范围、结果和日志路径；无需为小任务另建账本。
- 代码、fixture、预期、依赖、工具或环境的相关变化才使证据失效。收尾、换 agent、上下文压缩或无关文件变化不自动触发重跑；排查不稳定性时可有目的地重复并说明要验证的假设。
- 长输出完整落到 `tmp/test-governance/`，保留原命令退出码；完成后只回传耗时、结果摘要、新失败与日志路径，诊断时再读取有界错误上下文。不把运行命令直接接到 `head`，避免截断进程或误读管道退出码。
- 等待使用已有进程句柄和有界状态查询，不反复输出完整编译命令、相同 warning 或大段日志。状态未变时不重新探索或扩大验证范围；进度沟通只说明新信息和待取得证据。

## Test Authenticity

- 预期来自已确认需求、协议、DTO 或领域不变量，不能仅从当前实现反推。测试绑定真实 route / service / component 或领域入口；mock 调用次数、占位元素和固定字符串不能单独结算业务行为。
- 回归与高风险测试用旧行为失败、受控错误分支或适当的 mutation 验证识错能力；不默认运行全量 mutation，也不要求每个低影响改动制造红灯。
- 后端 API 变化覆盖受影响的 status、响应字段、错误 shape、权限与状态副作用；未变化的中间件契约可复用既有证据。纯领域规则用单元测试，跨 DTO / 认证 / 错误映射边界时优先 route / service integration。
- 认证测试复用项目 session / CSRF fixture，不绕过 ACL；运行态认证取证遵循项目 `page-debug` / `api-debug` 规则。字段断言沿用后端 DTO 原名。
- 不用放宽断言、跳过分支、伪造 fixture 或 coverage ignore 消除失败。相同契约已有集成证据时，仅在真实环境、认证链或配置差异仍待确认时补运行态取证。

## Delivery Evidence

说明覆盖了哪些目标、实际运行或复用了什么证据、关键测试如何识错，以及未验证范围和原因。区分源码门禁、编译检查、行为测试和运行态结果；测试已编写、已编译或进程退出成功但未执行目标用例，都不等于行为测试通过。进入验收时使用 `qa-evaluation`，复用仍有效的证据。
