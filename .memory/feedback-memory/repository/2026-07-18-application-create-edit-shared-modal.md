---
memory_type: feedback
feedback_category: repository
topic: 应用新增与编辑共享弹窗表单
summary: 应用新增和编辑必须由同一个弹窗组件与表单定义承载；不要把编辑另做成 Drawer，字段差异通过明确的 create/edit contract 表达。
keywords:
  - application
  - create
  - edit
  - modal
  - drawer
  - shared form
match_when:
  - 修改应用新增或编辑入口
  - 设计 Settings 应用管理单行详情与编辑交互
  - 判断同一业务对象新增和编辑是否应拆成不同容器
created_at: 2026-07-18 18
updated_at: 2026-07-18 18
last_verified_at: 2026-07-18 18
decision_policy: direct_reference
scope:
  - web/app/src/features/applications/components
  - web/app/src/features/settings/components/application-management
---

# 应用新增与编辑共享弹窗表单

## 规则

应用新增与编辑使用同一个弹窗组件和同一份字段定义，不为编辑单独建立 Drawer 或第二套表单。组件使用显式 create/edit 模式表达差异：创建可选择应用类型和触发器冻结字段；编辑只读展示创建后不可变字段，并允许修改名称、描述、图标、标签和 schedule 配置。

## 原因

新增和编辑属于同一 Application 配置能力。分成 Create Modal 与 Edit Drawer 会让字段、校验、默认值和后续能力再次漂移，也使用户对同一对象形成两套交互心智。

## 适用场景

应用工作台新增、Settings 应用管理编辑、应用基本配置入口，以及后续新增应用字段时的表单归属判断。
