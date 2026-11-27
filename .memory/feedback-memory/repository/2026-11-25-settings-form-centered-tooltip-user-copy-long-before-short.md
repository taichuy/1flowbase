---
memory_type: feedback
feedback_category: repository
topic: 设置表单居中布局、小问号提示、面向用户文案与先长后短排布
summary: 设置页与插件配置表单须水平居中承载，字段解释收进标签旁小问号 Tooltip（禁堆砌大段底层实现灰字或与字数统计重叠），文案面向最终用户而非开发者，控件顺序遵循“先长输入、后短开关”
keywords:
  - settings
  - form-layout
  - tooltip
  - user-copy
  - plugin-settings
match_when:
  - 设计或修改 /settings/* 系统设置页、插件 ui/settings.tsx 或配置表单交互与文案时
created_at: 2026-11-25 23
updated_at: 2026-11-25 23
last_verified_at: 2026-11-25 23
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/
  - runtime-extensions/@*/**/ui/settings.tsx
---

# 设置页表单居中、小问号提示、面向用户文案与先长后短排布

## 时间

`2026-11-25 23`

## 规则

1. **布局居中**：宽屏设置工作区中的单列配置表单应放入水平居中的卡片容器（如 `maxWidth: 680` + 居中对齐），避免贴左悬空导致右侧大片留白失衡。
2. **字段说明收进小问号 Tooltip**：字段补充说明默认做成标签旁的小问号悬浮提示（`QuestionCircleOutlined` + `Tooltip`），不在每个控件下方铺设多行 `extra` 灰字，严禁与 `Input.TextArea` 的 `showCount` 等底部元素发生文字重叠。
3. **文案面向最终用户**：界面标题、简介与提示语只描述用户可感知的功能效果，严禁出现“静态 JSON 持久化”“无需查询数据库”“公共入口”“HTML / `<title>`”等面向开发人员的底层实现术语。
4. **控件顺序先长后短**：长文本输入框（`Input` / `TextArea`）排在上方，短布尔开关（`Switch`）集中排在下方。

## 原因

贴左非居中、大段技术术语说明、上方短开关与下方长输入框混排，以及 `TextArea` 字数统计与 `extra` 重叠，会导致商业产品交付给客户时观感粗糙且增加理解负担。

## 适用场景

`1flowbase` 宿主 `/settings/*` 配置面板、`1flowbase-plugins-business` 与 `1flowbase-official-plugins` 的 `ui/settings.tsx` 配置页，以及同类后台参数表单。
