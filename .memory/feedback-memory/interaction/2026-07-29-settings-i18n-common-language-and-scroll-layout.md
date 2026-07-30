---
memory_type: feedback
feedback_category: interaction
topic: Settings 多语言页使用通用命名与可滚动管理布局
summary: 用户明确要求多语言后台使用“多语言”等成熟用户语言，参考应用管理复用既有 Settings 管理表格布局并支持双向滚动；导航、分页已表达的信息不再用纯文本状态区重复占据页面空间。
keywords:
  - settings
  - i18n
  - language
  - admin ui
  - scrolling
  - redundant status
  - concise actions
  - application management
  - global english key
match_when:
  - 命名或调整 Settings 多语言入口与页面标题
  - 多语言目录页面出现纵向或横向滚动缺失
  - Settings 管理表格需要决定复用边界
  - Settings 管理页出现只重复导航、说明或总数的纯文本区域
  - Settings 工具栏按钮重复页面对象或实现术语
  - 动态目录要决定 key、module 与语言翻译的身份关系
created_at: 2026-07-29 22
updated_at: 2026-07-30 10
last_verified_at: 2026-07-30 10
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - web/app/src/shared/ui/data-table
---

# Settings 多语言页使用通用命名与可滚动管理布局

## 规则

- 面向管理员的入口和页面标题使用“多语言”等成熟通用名称，不暴露“动态翻译管理”这类实现术语。
- 多语言目录参考 `/settings/applications` 的工具型后台布局，优先复用已有 Settings 页面壳与 DataTable。
- 长列表必须有明确的纵向滚动 owner；宽表必须能横向滚动，不能通过压缩所有字段假装适配。
- 导航已表达页面名称、分页已表达总数时，不再额外渲染只承载标题、说明和总数的纯文本状态区。
- 工具栏上下文已经明确时，按钮使用“筛选 / 恢复默认值 / 新建”这类最短无歧义任务动词，不重复“翻译”“官方翻译”“自定义翻译键”等对象说明。
- 调整按钮文案时同步核对中文、英文、key 与静态引用；owner 内已有语义足够稳定的通用短词 key 时直接复用，不能只改中文 value 或为相同英文 value 新增专用 key。本页新增动作最终复用 Settings `auto.new`（新增 / New）。
- 筛选条件与页面操作不要挤在同一行：筛选条件独占第一行并左对齐，操作按钮独占第二行并右对齐；移动端筛选项可纵向展开，但按钮仍保持成组右对齐。
- 不为单页再造只转发参数的通用 wrapper；现有公共组件足以承载时直接组合复用。
- 动态目录的 key 使用开发期固定的英文原文，并在目录内全局唯一；`en_US` 也必须保存独立、可覆盖的英文翻译，原始 key 只承担解析完全失败时的最终兜底。同一 key 对同一语言只有一个全局翻译，管理员修正后所有消费者共同生效。不要引入 `file.metadata` 这类变量式 key，也不要用 module 区分相同英文 key。

## 原因

- 用户按任务理解入口，不应先理解动态目录、覆盖层等内部机制。
- 固定视口壳如果没有内部滚动 owner，会直接裁掉分页与表格内容；宽表无横向滚动则降低可读性和管理效率。
- 重复纯文本不能帮助用户完成筛选、浏览或编辑，只会增加表格前的垂直距离。
- 精确动作词能降低扫描成本，也更容易让筛选控件与操作按钮保持紧凑排列。
- 两行分区能稳定表达“查询条件”和“页面操作”两类职责，避免为了塞进单行而压缩控件宽度或破坏视觉节奏。
- 当英文原文本身承担稳定 key 时，额外 module 会把来源组织方式泄漏进数据库、API 和人工新增流程；源文件仍可分目录维护，但分组不参与翻译身份。

## 适用场景

- `/settings/i18n` 及同类 Settings 资源管理页面。
- 页面需要在固定视口 Settings 壳内承载筛选、表格、分页和抽屉操作。
