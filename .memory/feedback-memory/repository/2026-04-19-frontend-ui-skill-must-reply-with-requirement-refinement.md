---
memory_type: feedback
status: superseded
feedback_category: repository
topic: frontend-development skill 对 UI 开发需求必须先输出含交互设计的需求整理
summary: 已被 2026-09-08 用户确认的前端 skill 收敛方案取代；原文只保留历史背景，不再用于恢复需求模板或默认实现链路。当前执行规则见 frontend-development 与 problem-framing。
keywords:
  - frontend
  - skill
  - ui requirement
  - requirement refinement
  - customer-facing reply
  - interaction design
  - quick reference
  - general workflow
  - trigger conditions
  - examples
  - reference index
  - implementation anchors
match_when:
  - 更新 `frontend-development` skill
  - 设计页面 / UI 开发类 skill 的触发条件和默认回复
  - 判断 UI 开发需求是否必须先输出需求整理
  - 重构 skill 主文件结构
  - 补齐需求整理的方法论或示例入口
  - 调整 quick reference 与 references 的职责边界
  - 调整 implementation 与 quick reference 的职责边界
created_at: 2026-04-19 00
updated_at: 2026-09-08 08
last_verified_at: 2026-09-08 08
decision_policy: verify_before_decision
scope:
  - .agents/skills/frontend-development
  - .memory/feedback-memory/repository
---

## Superseded

2026-09-08 08：用户确认直接更新前端 skill、无需 issue。需求决策统一由 `problem-framing` 承接，已确认方向或直接实现授权不重复需求模板；前端保留交互证据卡。以下旧阶段正文与旧路径仅供追溯，不作为当前执行规则。当前入口：`.agents/skills/frontend-development/SKILL.md`。


# frontend-development skill 对 UI 开发需求必须先输出含交互设计的需求整理

## 时间

`2026-04-19 07`

## 规则

- `frontend-development` skill 命中页面 / UI 开发需求时，回复里必须先显式输出需求整理、需求细化、页面交互设计和明确建议。
- 这份需求整理是发给用户 / 客户看的必要内容，不是只在内部完成。
- 新页面、页面改版、布局调整、模块级 UI 开发、图片 / 外部样本驱动时，应使用完整需求草案。
- 明确范围的页面 / 模块 UI 开发，至少也要给简版需求整理。
- 需求整理不能只停留在页面结构和模块列表，必须先整理主路径、关键反馈和模块协作，再开始实现。
- 只有纯局部样式修补、像素级对齐、文案替换或不改变页面结构的 UI bugfix，才可以跳过完整需求整理。
- skill 主文件应包含一段通用工作流程，先说明默认执行顺序，再补场景化快速引用。
- 主 skill 里的 `Quick Reference` 只负责快速引用目录和入口导航，不承载详细规范正文。
- 详细规范、方法论和场景说明应下沉到 `references/`，由 `Quick Reference` 指向对应文件。
- `Implementation` 只负责具体落地锚点、对象链路和验证链路，例如 `DESIGN.md`、目录落点、节点链路、样式链路、验证链路，不再重复 references 目录。
- `When to Use` 只应描述什么场景触发这个 skill，不应重复“先整理需求 / 是否先问人 / 直接做还是复用”这类流程规则。
- 需求整理流程不应只写“先整理”，还应显式指向方法论、模板和示例入口，例如 `requirement-refinement.md`、`extraction-framework.md`、`skill-template.md` 和 `examples/`。

## 原因

- 如果只在脑内做需求收敛，用户看不到 AI 的任务理解和边界判断，容易误以为 AI 直接按自己的想象写页面。
- 触发条件只写“模糊需求”太窄，会漏掉很多其实也应该先整理需求的 UI 开发请求。
- 把需求整理作为显式回复，可以更早暴露方向偏差，同时不牺牲默认继续实现的速度。
- 如果需求整理不包含页面交互，agent 很容易退化成“把几个卡片堆上去”，没有真正设计用户如何完成任务。
- 如果主文件没有通用工作流程，后续 agent 容易只抓局部规则，不理解默认执行顺序。
- `Quick Reference` 如果继续堆完整正文，会降低主 skill 的可扫描性，也更容易和 `references/` 里的正文重复。
- 把详细规范下沉到 `references/` 后，主 skill 更像入口索引，后续 agent 也更容易按任务去对应文件取方法。
- 如果 `Implementation` 继续重复 references 文件名，就会和 `Quick Reference` 重新长成两层重复目录，失去“具体怎么落”的价值。
- 如果 `When to Use` 混入流程规则，会和通用工作流程、communication gate、requirement refinement 发生重复甚至冲突。
- 如果主 workflow 不指向方法论和示例入口，后续 agent 容易只记住“要整理需求”，但不知道该按哪套方法整理。

## 适用场景

- 调整 `frontend-development` skill 的 metadata、默认动作或模板
- 处理页面开发、页面改版、模块级 UI 开发需求
- 判断 AI 是否应该先向用户输出需求细化和交互设计再开始实现
- 重构 skill 的通用工作流程
- 精简或重构 skill 的 `Quick Reference`
- 清理 skill 的触发条件与流程规则边界
- 补齐需求整理的方法论、模板和示例入口
- 调整 `Quick Reference` 与 `references/` 的职责边界
- 调整 `Implementation` 与 `Quick Reference` 的职责边界
