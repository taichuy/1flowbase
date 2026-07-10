---
memory_type: project
topic: 多语言统一 locale 合同与 i18n hygiene 门禁
summary: 用户最终确认多语言格式转换不保留前端/后端两套 locale。全仓 canonical locale 统一为 `zh_Hans` 与 `en_US`；前端 App/UI 资源、用户偏好、API locale、插件 provider catalog 都使用同一套 locale。`i18n-hygiene` 是开发与 QA 门禁，用于帮助 AI 复用同 owner 既有 key。
keywords:
  - i18n
  - locale
  - hygiene
  - frontend
  - provider
  - qa
match_when:
  - 需要做多语言格式转换
  - 需要新增或调整 i18n 资源
  - 需要判断 locale 命名边界
  - 需要解释 `i18n-hygiene` 门禁用途
created_at: 2026-05-28 20
updated_at: 2026-05-28 21
last_verified_at: 2026-05-28 21
decision_policy: verify_before_decision
scope:
  - .agents/skills/frontend-development/references/i18n-rules.md
  - .agents/skills/qa-evaluation/references/frontend/i18n-hygiene-gate.md
  - scripts/node/i18n-hygiene
---

# 多语言统一 locale 合同与 i18n hygiene 门禁

## 时间

`2026-05-28 21`

## 谁在做什么

用户准备做多语言格式转换。AI 先检查现有策略、前端 i18n 实现、后端 locale 解析、provider catalog 和 `i18n-hygiene` 门禁后，最初建议保留双 locale 体系。用户随后指出前端刚起步，直接适配后端更好，没必要维护两套；最终确认全仓统一使用 `zh_Hans` 与 `en_US`。

## 为什么这样做

前端多语言资源刚起步，迁就后端 canonical locale 的改动成本低；后端 profile/API locale、用户偏好和插件 provider catalog 已经使用 `zh_Hans` 与 `en_US`。统一后可以减少边界转换、资源文件名分叉和 AI 判断成本。

## 为什么要做

多语言格式转换容易把“资源文件名格式”“运行态 locale”“接口字段名”和“展示文案”混在一起。统一 canonical locale 可以避免为了展示便利新增 DTO 别名字段，也避免 AI 在前端/后端 locale 命名之间来回转换。

## 截止日期

无

## 决策背后动机

让后续 AI 在新增文案或做格式转换时先通过 `i18n-hygiene` 查同 owner 既有 key / value，再决定复用、调整文案或保留局部 owner；error 必修，warning 需要解释保留或上提理由。URL、浏览器语言和 `Accept-Language` 可以接受 `zh-CN`、`zh-Hans`、`en-US`、`en` 等别名，但进入系统后必须归一化为 `zh_Hans` 或 `en_US`。

## 已落地规则

- `.agents/skills/frontend-development/references/i18n-rules.md`
- `.agents/skills/qa-evaluation/references/frontend/i18n-hygiene-gate.md`
