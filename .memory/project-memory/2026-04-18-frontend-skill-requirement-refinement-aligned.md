---
memory_type: project
status: superseded
topic: frontend-development skill 已补模糊需求与图片参考的需求收敛流程
summary: 已被 2026-09-08 用户确认的前端 skill 收敛方案取代；原文只保留历史背景，不再用于恢复需求模板或默认实现链路。当前执行规则见 frontend-development 与 problem-framing。
keywords:
  - frontend
  - skill
  - requirement refinement
  - image reference
  - design brief
match_when:
  - 需要使用或更新 `frontend-development` skill
  - 用户只给图片、截图、竞品页或模糊页面需求
  - 需要判断前端任务是否应先澄清再实现
created_at: 2026-04-18 16
updated_at: 2026-09-08 08
last_verified_at: 2026-09-08 08
decision_policy: verify_before_decision
scope:
  - .agents/skills/frontend-development
  - .memory/project-memory
---

## Superseded

2026-09-08 08：用户确认直接更新前端 skill、无需 issue。需求决策统一由 `problem-framing` 承接，已确认方向或直接实现授权不重复需求模板；前端保留交互证据卡。以下旧阶段正文与旧路径仅供追溯，不作为当前执行规则。当前入口：`.agents/skills/frontend-development/SKILL.md`。


# frontend-development skill 已补模糊需求与图片参考的需求收敛流程

## 时间

`2026-04-18 16`

## 谁在做什么

用户要求把“很多前端需求都很模糊，或者直接丢一张图片进来”的场景补进 `frontend-development` skill。随后用户进一步修正：这套流程不应要求用户先确认，而应作为前端 UI 开发的前置步骤，AI 先收敛需求后默认直接开始实现，用户若觉得方向不对会自行打断。

## 为什么这样做

仅有“新页面先问人”的规则还不够细。真实对话里更常见的是用户给一句模糊目标或一张参考图，如果没有明确工作流，agent 很容易把第三方视觉样本直接当需求照搬，或者在页面目标、关键动作和状态都没收敛时就开始实现；但如果每次都停下来等确认，又会拖慢前端 UI 迭代。

## 为什么要做

需要让未来 agent 在命中模糊需求或图片参考时，先输出设计需求草案、说明“借什么 / 不借什么”，然后默认继续实现；只有遇到无法自行判断的阻塞性产品决策时，才一次性集中提问。这样能把外部样本重新翻译回 1flowbase 自己的 `DESIGN.md`、页面 recipe 和状态语义，同时保持前端 UI 迭代速度。

## 截止日期

无

## 决策背后动机

当前项目前端正在持续建立稳定的工作台语法。把这套需求收敛流程放进 `frontend-development` skill，而不是只靠临场判断，可以减少“抄参考图皮肤”“拿模糊需求直接写页面”这类高频偏航，并让 agent 在相似场景下更一致地先澄清、再默认继续实现。

## 关联文档

- `.agents/skills/frontend-development/SKILL.md`
- `.agents/skills/frontend-development/references/communication-gate.md`
- `.agents/skills/frontend-development/references/requirement-refinement.md`
- `.agents/skills/frontend-development/references/review-checklist.md`
