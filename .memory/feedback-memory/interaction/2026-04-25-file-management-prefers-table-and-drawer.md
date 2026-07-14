---
memory_type: feedback
feedback_category: interaction
topic: 后台文件管理页优先表格管理与抽屉表单
summary: 用户明确指出后台管理页不接受占据首屏的 hero 标题大卡片；无合法消费方的共享 UI 能力应直接删除，不保留 bool 开关后再靠 AGENTS 或禁止测试约束。
keywords:
  - file management
  - table
  - drawer
  - admin ui
  - settings
  - hero header
  - SettingsSectionSurface
  - hideHeader
match_when:
  - 后台管理页出现卡片墙
  - 文件管理、配置管理、资源管理页需要改版
  - 用户要求列表管理与详情编辑并存
  - 新增或注册 Settings 后台页面
  - 页面首屏出现只承载标题和描述的大卡片
created_at: 2026-04-25 08
updated_at: 2026-07-15 07
last_verified_at: 2026-07-15 07
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - .memory/feedback-memory/interaction
---

# 后台文件管理页优先表格管理与抽屉表单

## 时间

`2026-04-25 08`

## 规则

- 文件管理这类后台管理页，列表主体优先使用表格，不使用卡片墙式展示。
- Settings 后台 section 默认直接进入工具栏和主体，不显示只承载页面名与一行说明的 hero 标题大卡片。
- hero 若没有合法消费方，直接从共享组件中删除 props、DOM、CSS 和废弃文案；不保留 `hideHeader` 之类反向 bool 能力。
- 已能由类型系统、实际页面测试和 style-boundary 脚本约束时，不再往 `web/AGENTS.md` 添加同义规则，也不新增只扫描某个 prop 字符串的禁止测试。
- 顶部工具条统一承载新增、刷新、检索。
- 单行操作统一放查看、编辑、删除。
- 新增、查看、编辑统一使用抽屉承载表单或详情。

## 原因

- 卡片墙在后台管理页的信息密度和操作效率都偏低，不利于批量扫描和稳定管理。
- hero 标题大卡片重复了左侧导航已经表达的当前位置，挤压筛选和表格的首屏空间，与工具型控制台密度冲突。
- 保留无业务价值的开关再靠文档禁止，会同时增加 AI 误用诱因和规则维护成本；结构性删除能让错误用法直接无法编译。
- 表格配合抽屉更符合配置型页面的主任务路径：先筛选定位，再对单条记录处理。

## 适用场景

- 设置页中的资源管理、配置管理、文件管理
- 新增 Settings 路由、后台设置注册页和 `SettingsSectionSurface` 消费方
- 需要头部工具条和单行操作的后台页面
- 详情编辑不需要跳转独立页面的管理界面
