---
memory_type: feedback
feedback_category: repository
topic: problem-framing 应追问关键隐含问题并以因果机制控制拆解深度
summary: 需求对齐要主动指出用户未问但可能更关键的问题；拆解以是否讲清因果机制为标准，不以抽象层级数量为标准。权限、注册与扩展设计必须同时覆盖后续开发者入口、默认安全行为、开源外部部署历史数据和可执行质量门禁。Skill 正文只保留稳定流程和读取条件，完整 issue 格式放入 references 模板。
keywords:
  - problem-framing
  - causal mechanism
  - hidden question
  - issue template
  - progressive disclosure
match_when:
  - 调整或使用 problem-framing
  - 需求拆解出现空洞抽象层级
  - Skill 正文包含完整产物格式或模板
created_at: 2026-07-10 10
updated_at: 2026-07-13 17
last_verified_at: 2026-07-10 10
decision_policy: direct_reference
scope:
  - .agents/skills/problem-framing/SKILL.md
  - .agents/skills/problem-framing/references
  - requirement alignment
---

# Problem Framing Causal Depth And Template Boundary

## 时间

`2026-07-10 10`

## 规则

- 需求对齐时，主动找出用户当前问题背后“没有直接问、但更可能决定方案成败”的关键问题，并明确指出其与原问题的因果关系。
- 拆解深度以能否解释现象、约束、决策与结果之间的因果机制为停止标准；不能增加判断力、验收力或行动信息的抽象层级应删除。
- 讨论权限、注册、插件或扩展架构时，必须同时回答后续开发者怎样注册、遗漏时系统是否默认拒绝、哪些状态落库、哪些门禁能枚举并证明覆盖；不能只给运行时结构或当前 UI 方案。
- 开源项目中“当前团队无人使用”不能推导为“没有历史数据”；只要功能进入过已发布版本，就必须按外部部署可能已有数据评估 migration。可以直接替换旧运行时 contract，但不能跳过历史数据转换、零差异检查和失败回滚。
- `SKILL.md` 正文只保留稳定原则、阶段闸门、选择规则和何时读取参考文件；完整 issue 字段、Markdown 骨架和示例放在 `references/` 模板中，避免正文与模板双重维护。

## 原因

用户指出，深度不等于层级多。空洞抽象会让需求看似完整，却没有解释真正的驱动因素，也不能改善方案选择。2026-07-13 用户进一步指出，权限方案若没有覆盖后续开发维护、统一鉴权入口和自动检测门禁，就遗漏了决定长期可靠性的核心问题；同日补充，内部使用状态不能代表开源外部部署，已发布数据必须纳入 migration。完整 issue 格式属于按需使用的产物模板，不应挤占每次触发 Skill 都会加载的核心正文。

## 适用场景

- 普通需求对齐、缺陷根因分析、架构或产品方向讨论。
- 输出讨论 brief、issue draft、ADR 或 implementation handoff 前。
- 创建或调整项目 Skill 的正文与 references 信息架构。
