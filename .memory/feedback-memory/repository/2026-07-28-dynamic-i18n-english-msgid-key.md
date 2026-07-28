---
memory_type: feedback
feedback_category: repository
topic: dynamic-i18n-english-msgid-key
summary: 动态多语言 key 必须是可直接展示的英文原文，而不是 `restore_defaults` 或带 namespace 的机器标识；目标 locale 查不到翻译时原样显示英文 key。目录/module 提供上下文，避免把组织和模块编码进展示 fallback。
keywords:
  - i18n
  - msgid
  - english-key
  - fallback
  - gettext
match_when:
  - 设计动态多语言 key、引用语法或缺失翻译行为
  - 设计官方 i18n JSON、管理后台新增 key 或低代码 i18n_ref
  - 判断 namespace、module 与可展示 fallback 的关系
created_at: 2026-07-28 17
updated_at: 2026-07-28 17
last_verified_at: 2026-07-28 17
decision_policy: direct_reference
scope:
  - ../1flowbase-official-plugins/i18n
  - api
  - web/app/src/features/settings
---

# Dynamic i18n English Msgid Key

## 规则

- key 使用可直接展示的英文源文本，例如 `Restore defaults`。
- 目标语言翻译不存在或为空时，结果就是英文 key 本身。
- namespace/module/context 与英文 key 分开保存；不得把 `@taichuy.core.settings:` 等内部标识拼进最终展示 fallback。
- 面向作者可采用常见的 `t("Restore defaults")`；持久化时使用 typed reference，并由所在模块或显式字段承载 namespace。

## 原因

机器标识 key 在缺失翻译时会暴露内部命名，违背用户希望“无翻译也至少显示可读英文”的目标。英文 msgid 机制把默认内容与引用合一，模块上下文再解决同一句英文在不同语境下的翻译差异。

## 适用场景

- 官方动态多语言源文件与 catalog manifest。
- 后台新增、编辑、还原动态 key。
- 低代码或后端 DTO 引用动态多语言。
