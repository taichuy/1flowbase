---
memory_type: feedback
feedback_category: repository
topic: frontend-development skill 遇到交互架构决策时必须先跑 interaction architecture gate
summary: 用户确认 `frontend-development` 是前端交互与结构设计的统一 Skill；只要存在交互架构决策就先过 interaction architecture gate。独立 `frontend-logic-design` Skill 已于 2026-07-10 移除，结构性问题由 gate 继续完成完整诊断，并用 references、checklist、examples 和 `web/AGENTS.md` 固化执行路径。
keywords:
  - frontend
  - skill
  - interaction architecture
  - gate
  - frontend-development
  - trigger conditions
  - examples
  - checklist
match_when:
  - 更新 `frontend-development` skill 的交互设计规则
  - 设计前端 skill 的入口、层级、详情容器和 L0/L1/L2/L3 触发条件
  - 补充前端 skill 的 gate、示例、checklist 或 `web/AGENTS.md`
created_at: 2026-04-19 00
updated_at: 2026-07-10 00
last_verified_at: 2026-07-10 00
decision_policy: direct_reference
scope:
  - .agents/skills/frontend-development
  - web/AGENTS.md
  - .memory/feedback-memory/repository
---

# frontend-development skill 遇到交互架构决策时必须先跑 interaction architecture gate

## 时间

`2026-07-10 00`

## 规则

- `frontend-development` 是前端实现、交互和结构设计的统一入口，不能把交互设计只当成普通页面细化。
- 只要任务涉及入口、层级、详情容器、`查看全部`、AI 执行落点、`L0 / L1 / L2 / L3` 或同类对象行为统一，就必须先运行交互架构 gate。
- 独立 `frontend-logic-design` Skill 不再保留；gate 判断问题进入结构性设计时，在 `frontend-development` 内完成完整诊断。
- 交互架构 gate 负责触发信号、快审、完整诊断条件和对用户可见的输出模板；存在产品级未决取舍时回到 `problem-framing`。
- `frontend-development` 的 `references`、`review-checklist`、`examples` 和 `web/AGENTS.md` 都要显式反映这条 gate 路径，避免主 Skill 提一句但执行时漏掉。
- 示例不只要写“页面结构”和“关键状态”，还要示范首屏主任务、L1 / L2 / L3、反馈落点和一致性规则怎么说。

## 原因

- 前端任务大多是设计和实现混合，拆成两个 Skill 会提高触发和切换成本。
- 如果不把 gate 写进主 workflow、沟通门槛、复查清单和示例，agent 很容易在实现压力下忽略交互架构判断。
- 让 `frontend-development` 直接承接快审与完整诊断，可以减少 Skill 切换，同时保留结构设计约束。

## 适用场景

- 重构 `frontend-development` Skill 的 workflow、trigger、reference 或 example
- 判断某个前端任务是否属于交互架构决策而不是普通页面细化
- 补充前端 Skill 的 checklist、communication gate 或 `web/AGENTS.md`
- 评估为什么 agent 在前端任务里“会谈交互，但不真正做交互设计”
