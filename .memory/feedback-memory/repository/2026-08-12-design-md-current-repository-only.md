---
memory_type: feedback
feedback_category: repository
topic: DESIGN.md 只针对当前仓库，不以外部来源作为设计依据
summary: 优化 1flowbase DESIGN.md 时，只以当前仓库的产品语义、已确认界面、代码 token 和本地约束为依据；登录页是品牌色调参考，用户认可并释放到 shared/ui 的共享样式是后台组件参考；外部原始来源不参与判断。
keywords:
  - DESIGN.md
  - design system
  - current repository
  - external reference
match_when:
  - 优化或审计根目录 DESIGN.md
  - 从外部设计仓库追溯 1flowbase 视觉规范来源
created_at: 2026-08-12 22
updated_at: 2026-08-12 22
last_verified_at: 2026-08-12 22
decision_policy: direct_reference
scope:
  - DESIGN.md
  - web
---

# DESIGN.md 只服务当前仓库

## 时间

`2026-08-12 22`

## 规则

优化、重构或验收 1flowbase 根 `DESIGN.md` 时，只根据当前仓库的产品基调、已确认页面、代码 token、共享组件和本地规则作判断。即使用户指出外部仓库是早期来源，也不得据此重新套用其模板、跟随其演进，或把它加入当前设计依据。

具体取证优先级：登录页面用于提炼品牌主色、环境色与氛围；用户认可并已释放到 `web/app/src/shared/ui/` 的共享样式用于提炼后台组件、容器层级和页面组合。不要让旧 `DESIGN.md` 反向否定这些已确认的当前样式。

## 原因

`DESIGN.md` 是当前 1flowbase 仓库的设计文档。历史来源只能说明形成过程，不能取代已经长期演进并定型的本地产品事实；引入外部范式会扩大范围并扭曲当前目标。

## 适用场景

根设计文档更新、前端视觉基调审计、Design Token 对齐、页面 Recipe 与 Agent 设计约束整理。
