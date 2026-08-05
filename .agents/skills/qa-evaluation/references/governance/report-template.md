# QA Report Template

## Scope

- 当前评估模式：
- 评估范围：
- 输入来源：
- 已运行的验证：
- 未运行的验证：

## Gate Lane

- 当前 lane：
- lane 选择理由：
- 资源预算与停止条件：
- 是否存在当前失败脚本 / 错误报告输入：
- 该失败输入的归属：证据来源，不作为完整评估范围

## Foundation Contract Evidence

- 命中的基座 / 组合缝隙：
- candidate SHA 与触发原因：
- fast pack 结论：
- warning / error / blocker：
- 未覆盖项与 nightly/manual 延后证据：

## Coverage Matrix

- 适用范围：`Project Health Gate` 必填；其他 lane 可写不适用
- 维度：
- 覆盖证据：
- 当前结论：
- 未覆盖项：
- 本轮轮转深挖域：

## Acceptance Point Settlement

| 编号 | 结论 | 证据 | 残余风险 |
| --- | --- | --- | --- |
| AC-001 | green / red / 未验证 |  |  |

## Evidence Classification

- 自动化门禁 / CI / artifact：
- 当前失败脚本 / 日志：
- 运行态 / 截图：
- 代码 / 契约 / 状态证据：
- 记忆 / spec / 历史趋势证据：
- 归因：硬性失败 / warning / advisory / 未覆盖

## Code Audit Evidence

- Candidate identity：commit / branch / dirty diff / receipt candidate：
- Artifact freshness：generated_at / candidate SHA / environment / stale or current：
- Rule：命中的明确规则或不变量：
- Evidence：文件、调用点、运行路径、fixture、日志、plan 或 artifact：
- Impact：当前可观察影响与 blast radius：
- Legal negative：已检查的合法反例，以及为什么不适用：
- Severity：Blocking / High / Medium / Low warning / Advisory：
- Unverified：缺失证据、环境限制和结论降级：
- Authorization：只报告 / 已授权修复 / 需新 Issue：

## Conclusion

- 是否存在 `Blocking` 问题：
- 是否存在 `High` 问题：
- 当前是否建议继续推进：
- 当前最主要的风险：

## Context Capsule

- Built:
- Lives in:
- Decisions / gotchas:
- Extend from:

## Findings

### [Severity] [Title]

- 位置：
- Rule：
- Evidence：
- Impact：
- Legal negative：
- Candidate identity / Artifact freshness：
- Unverified：
- 建议修正方向：

### [Severity] [Title]

- 位置：
- 证据：
- 为什么是问题：
- 建议修正方向：

## Warnings

### [Low warning] [Title]

- 位置：
- 证据：
- 为什么是风险：
- 建议修正方向：
- 修改授权状态：未授权，需用户明确同意

## Prevention Layer

- 这次反复修改暴露的 AI 前置判断缺口：
- 应更新的 skill：
- 应更新的 AGENTS / 本地规则：
- 应新增或调整的质量脚本 / 门禁：
- 下次同类任务进入实现前必须先问或先检查的事项：

## Uncovered Areas / Risks

- 因环境、权限、时间或范围限制未验证的项
- 因上下文缺口导致只能给出受限结论的项
- 若暂不修复 `Low` 问题，需要写清原因
