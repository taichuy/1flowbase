---
memory_type: project
topic: 动态后台多语言首期边界对齐
summary: 用户确认 bootstrap root workspace 的后端动态多语言方向；Root #1488 已完成 QA-5 并 fast-forward 集成到本地 beta / official main。Settings 自身静态 locale owner 保持不变，后台动态返回、接口元数据与低代码 i18n_text 走 PostgreSQL 动态目录；等待 push 与用户验收。
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
updated_at: 2026-07-29 00
last_verified_at: 2026-07-29 00
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
- 在线计划真值：Issue Tree Root #1488，D1 #1489、D2 #1490、D3 #1491；实现与 QA 已完成，等待用户验收。
- 执行状态：main candidate `77f04c7be554c62c831fc70a1d021974f61f11c9` 与 official `7fa2d7daf9c435daf389a238b220cb923f730fdc` 已分别 fast-forward 到本地 `beta` / official `main`；推送状态以 Git 远端为准。
- QA-5：official publisher 15/15；api-server i18n 22/22；storage 13/13；domain 4/4；control-plane 13/13；access-control 18/18；orchestration 6/6；app 39/39；API client 177/177；flow-schema 41/41；Chromium 动态筛选与 Settings desktop/mobile style-boundary 通过；i18n hygiene 0 errors。console route hygiene 仅保留与 beta 相同的 2 个既有 middleware errors，#1488 新增差异为 0。
- 边界校正：Settings 导航与 Settings feature permission DTO 继续返回静态 `label_key`；角色策略、接口 summary/description 和其他已冻结 backend consumer 使用独立 English msgid 动态投影，前端静态 locale 不迁移。
