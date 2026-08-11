---
memory_type: project
topic: Agent 分发跟踪学习平台 MVP 配置状态
summary: 已用 1flowbase 配置数据完成教育数据模型、Agent Flow、Workflow、Frontstage Block 页面和受限 MCP 目录；产品源码未修改。Extension 已发布，Schedule 保持停用，学生记录 Tool 因缺少 student_key 行级隔离证据未挂载。
keywords:
  - education
  - Agent distribution
  - learning trace
  - Frontstage
  - MCP
  - Workflow
created_at: 2026-08-10 20
updated_at: 2026-08-10 20
last_verified_at: 2026-08-10 20
decision_policy: verify_before_decision
scope:
  - workspace 00000000-0000-0000-0000-000000000001
  - education data models
  - education applications
  - education_learning MCP instance
---

# 教育 Agent 平台 MVP

Root agent 在工作区 `00000000-0000-0000-0000-000000000001` 用平台配置搭建教育场景 MVP，原因是用户要求以 1flowbase 开放能力做场景装配，而不是修改或生成产品代码。素材入口为 `docs/ziliao/xitonjiagoushi.zip` 与 `docs/ziliao/ppt-luyan.pptx`。

当前已发布三项教育数据模型：`education_student_profiles`、`education_diagnosis_reports`、`education_learning_plans`；已创建作业辅导与分层备课 Agent Flow、夜间诊断 Schedule Workflow、个性化规划 Extension Workflow。规划 Extension 已发布到 `/api/ex/education/learning-plan/generate`；夜间 Schedule 固定 `0 2 * * * / Asia/Shanghai` 且 `enabled=false`。

Frontstage 已有 `教学总览`、`教师诊断`、`学生学习` 三个顶栏页面，浏览器点击与 Block heading 已验证，证据在 `tmp/test-governance/education-frontstage-smoke/`。教师侧主 MCP 实例挂载 `education_generate_learning_plan` 于 `/education/plans`；独立学生实例 `education_learning` 已建立 `/learning/history`、`/learning/diagnosis`、`/learning/plans`，但在后端不能证明按 `student_key` 行级隔离前不挂载个人记录 Tool。

截止日期为本轮交付。继续扩展前必须重新验证 provider 可用性、MCP wrapper request_schema 缺口和学生数据授权 contract。
