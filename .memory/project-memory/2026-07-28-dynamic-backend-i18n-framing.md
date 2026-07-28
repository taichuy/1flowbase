---
memory_type: project
topic: 动态后台多语言首期边界对齐
summary: 用户于 2026-07-28 17 最终确认 bootstrap root workspace 的后端动态多语言方向、English msgid 改名语义和全局还原保留 custom key；前端静态 i18n 与 hygiene 门禁保持不动，不建立跨 scope 继承。GitHub Issue Tree Root #1488 已创建并成为后续计划、进度和用户验收的唯一在线真值。
keywords:
  - dynamic-i18n
  - backend
  - settings
  - system
  - workspace
  - cache
  - persistence
match_when:
  - 拆解或实现动态多语言后台管理
  - 设计多语言初始化、更新、还原或浏览器缓存
  - 判断是否迁移前端静态 i18n
created_at: 2026-07-28 16
updated_at: 2026-07-28 17
last_verified_at: 2026-07-28 17
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1488
  - ../1flowbase-official-plugins/i18n
  - api
  - web/app/src/features/settings
---

# 动态后台多语言首期边界对齐

## 谁在做什么

- 1flowbase 将新增后端动态多语言目录、持久化、版本更新、还原与后台管理能力。
- 前端继续保留现有静态 locale 文件；首期只为后端动态内容提供最小消费与管理页面，不迁移前端自身文案。

## 为什么这样做

- 前端静态 i18n 与 hygiene 门禁已经成熟，首期迁移会扩大风险但不增加后端动态目录的核心价值。
- 动态内容需要由后端统一拥有 root workspace 目录、版本、覆盖与缓存失效语义；目标语言缺失时直接使用英文 msgid，不建立跨 scope fallback。

## 为什么要做

- 消除后端与低代码元数据中的硬编码多语言，并支持官方默认版本、管理员覆盖、自定义 key、在线更新和还原默认值。

## 截止日期

- 无。

## 已确认决策与待闭合边界

- 已确认：canonical locale 继续为 `zh_Hans / en_US`。
- 已确认：key 本身是可直接展示的英文原文；引用语法必须明确且无歧义，目标语言没有翻译时原样显示英文 key。
- 已确认：官方更新保留用户覆盖；还原默认值删除覆盖并显露当前上游默认值。
- 已确认：前端静态 i18n 文件与现有 hygiene 门禁不在首期迁移范围。
- 已确认：首期只作用于 bootstrap root workspace；不做 system fallback、多 workspace 或 application scope。
- 已确认：未来多 workspace 也应先按请求所属 workspace 独立解析，不能默认用 system 或其他 workspace 文案补齐。
- 已确认：英文文案变化按“新 key + 旧 key obsolete”处理，不增加隐式 alias。
- 已确认：全局“还原默认配置”保留管理员新增的 custom key；custom key 删除是独立破坏性动作。
- 在线计划真值：Issue Tree Root #1488，当前 `phase:ready`。用户尚未要求执行；开始执行后先做一个双仓只读 Scout，再创建并挂接 D1/D2/D3。
