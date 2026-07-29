---
memory_type: project
topic: 动态后台多语言首期边界对齐
summary: 用户确认 bootstrap root workspace 的后端动态多语言方向；Root #1488 已完成 QA-5，Seed 1.2.0 修复已发布到 official main/tag 与主仓 beta。Settings 自身静态 locale owner 保持不变，后台动态返回、接口元数据与低代码 i18n_text 走 PostgreSQL 动态目录；等待用户最终验收。
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
updated_at: 2026-07-29 23
last_verified_at: 2026-07-29 23
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
- 在线计划真值：Issue Tree Root #1488，D1 #1489、D2 #1490、D3 #1491；实现与 QA 已完成，等待用户验收，不自动关闭 Root。
- 执行状态：official `main` 已推送 `69754721577e1d0af0e540a4bd2ee073ea110661` 并发布 tag `i18n-catalog-v1.2.0`；主仓 `beta` 已推送 Seed 修复/PR 门禁 `b76f93b4d89a80d3c1245431f4266a792a462e42` 与惰性启动加载 `c69ba94170c923916dc7f271055ba2e3a345f69c`。
- 2026-07-29 Seed 事故结论：主仓手改生成 Seed 但未同步文件/语义摘要和官方 provenance，api-server 启动 fail-fast，导致 Vite 代理 `login-instances` 返回 502，`/sign-in` 回退为空发现状态。修复方式是从 official publisher 生成 1.2.0、同步固定 commit/release metadata，并用数据库无关 Rust Seed 门禁阻止再次手改。
- 运行时边界校正：启动先检查 root workspace 是否已有 active catalog release；已有时完全跳过内嵌 Seed loader，缺失时才校验并通过既有事务原子导入。PR/构建 Seed 门禁保留；管理员在线更新和还原语义不变。
- 修复后运行证据：Seed 摘要/覆盖门禁通过；惰性 loader 测试 2/2、Bootstrap 回归 12/12、PostgreSQL 全新/升级/热重启场景通过；Vite 代理 `login-instances` 返回 200 和 3 个既有登录实例，热重启前后身份、默认项与顺序一致。未清库、未还原翻译覆盖。
- QA-5：official publisher 15/15；api-server i18n 22/22；storage 13/13；domain 4/4；control-plane 13/13；access-control 18/18；orchestration 6/6；app 39/39；API client 177/177；flow-schema 41/41；Chromium 动态筛选与 Settings desktop/mobile style-boundary 通过；i18n hygiene 0 errors。console route hygiene 仅保留与 beta 相同的 2 个既有 middleware errors，#1488 新增差异为 0。
- 边界校正：Settings 导航与 Settings feature permission DTO 继续返回静态 `label_key`；角色策略、接口 summary/description 和其他已冻结 backend consumer 使用独立 English msgid 动态投影，前端静态 locale 不迁移。
- 2026-07-29 23 页面体验增量：用户确认 Settings 入口改为“多语言 / Languages”，`/settings/i18n` 复用既有 `SettingsSectionSurface + DataTable`，不新增 wrapper 或修改全局 SectionPageLayout；后续按用户截图反馈移除重复标题、说明和修订统计状态区，将工具栏动作收紧为“筛选 / 恢复默认值 / 新建”，并冻结为筛选条件第一行左对齐、操作按钮第二行右对齐。最新定向测试 5/5、桌面/移动 style-boundary 与隔离运行时快照通过；真实 3200 路由复验因本地 API 7900 未运行、登录返回 502 未完成，等待用户最终验收。
