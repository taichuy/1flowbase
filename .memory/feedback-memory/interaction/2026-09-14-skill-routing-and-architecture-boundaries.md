---
date: 2026-09-14 00
feedback_category: interaction
decision_policy: direct_reference
summary: Skill 优化保留强制三方向与固定对齐格式，明确设计真值和开发/QA 架构检索职责。
---

## 规则与原因

- problem-framing 的三个方向和固定回复格式必须保留：它们用于发散、收敛、用户抉择及计划检索去重，不能作为流程冗余删除。
- DESIGN.md 是前端设计真值；skill 提供定位和应用方法，不维护平行设计真值。
- 后端 skill 应提供足够的后端架构认知，并指向 api/AGENTS.md、局部 AGENTS 和 docs/architecture/interface-lifecycle.md 获取细节；不要求先读全项目架构。
- QA 需要参照架构检查开发是否偏离，不能只依据实现者说明或测试绿灯验收。
- 当前 skills 优化讨论排除 github-solution-research，保留原有规则。

## 适用场景

评估和调整项目 skills 的检索、上下文、职责边界与数学表达时应用；简化以保留上述必要决策机制为前提。

## 决策可执行性纠正

用户指出仅保留三方向和标题不够：仍需讨论的问题应前置明确，三个方向应是具体方案与决策试探，最终建议要让用户点头即可执行。不能把“确认入口 / 字段后再计划”当作收敛结果，也不能用内部场景与规则路径表代替用户方案。适用于 problem-framing 的输出和场景验收；检查可执行性，不只检查格式。

## 最新决定

用户要求撤回本次对 problem-framing 的优化与后续可执行性补丁，恢复本轮优化之前的主文件和示例。之后不要继续调整该 skill，除非用户重新明确授权。前述输出建议不再作为修改该 skill 的授权。其他 skills 优化保留。
