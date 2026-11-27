---
memory_type: project
topic: 商业授权白牌插件 (@taichuy/commercial-license) 与宿主公共品牌投影交付
summary: 已完成 @taichuy/commercial-license v0.1.2 六平台签名发布（居中卡片 + 小问号提示 + 面向用户文案 + 先长输入后短开关布局），并在 1flowbase 宿主实现静态 JSON 无 DB 直读的 PublicSystemMetadata 白牌投影（本地与 taichuy-dev 已同步未提交）
keywords:
  - commercial-license
  - white-label
  - system-metadata
  - SystemBranding
  - PublicSystemMetadata
match_when:
  - 修改 /settings/system、/settings/commercial-license、GET /api/public/system-metadata、顶栏 HelpChromeMenu 或登录页白牌文案与 Powered by 展示时
created_at: 2026-11-25 23
updated_at: 2026-11-25 23
last_verified_at: 2026-11-25 23
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane-contracts/src/ports/resource_ports/system_branding.rs
  - api/crates/api/console-settings/src/routes/settings/branding/public_interface.rs
  - web/app/src/app-shell/AppShellFrame.tsx
  - web/app/src/features/auth/
  - /home/taichuy/git/1flowbase-plugins-business/runtime-extensions/@taichuy/commercial-license
---

# 商业授权白牌插件 (`@taichuy/commercial-license`) 与宿主公共品牌投影交付

## 时间

`2026-11-25 23`

## 谁在做什么

开发者与 AI 完成了私有商业化插件仓库 `1flowbase-plugins-business` 下的 `@taichuy/commercial-license`（`v0.1.1`，GitHub Release `commercial-license-v0.1.1`，注册表提交 `52f75b0`）开发与全平台签名发布，并在 `1flowbase` 主仓 `dev` 分支（本地 Windows 与远端 `taichuy-dev` 同步 `29` 个未提交文件）接入公共展示元数据合成支持。

## 为什么这样做

- **静态 JSON 直读、零 DB 查询、零常驻进程**：插件采用 `stdio_json` + `process_per_call`（二进制 `bin/cl-ext`，无数据库模型），配置原子写入版本无关家族目录 `{provider_install_root}/installed/commercial-license/config.json`（回退 `_assets/default-config.json`）；宿主 `GET /api/public/system-metadata` 在插件启用态下直接通过 `tokio::fs::read` 合成 `PublicSystemMetadata`，插件停用或卸载后自动恢复默认 `1flowbase` 展示。
- **与 `/settings/system` 完全正交无冲突**：保留 `/api/console/settings/system` 原 5 字段 `SystemBranding` 契约不变；`hide_login_tagline` 继续作为登录左屏大字唯一总显隐开关，商业插件仅负责内容覆盖（`page_title`、`login_hero_title`、`login_hero_tagline`）及独立白牌开关（`hide_help_menu`、`hide_powered_by`）。

## 为什么要做

满足商业化客户 OEM / 白牌交付场景：隐藏顶栏 `?` 帮助与 GitHub 入口、隐藏登录框底部 `Powered by 1flowbase`（不影响备案号 `filing_html` 且首屏不闪烁）、自定义浏览器标签页标题与登录左屏两行标语大字。

## 截止日期

无硬性截止日期；插件仓已完成线上 `v0.1.1` 发布，`1flowbase` 宿主改动已在本地与 `taichuy-dev` 验证通过，待合并提交 `dev`。

## 决策背后动机

遵循第一性原理与底座/插件职责边界：配置体量极小且读多写极少，静态 JSON 落盘由宿主公共接口直读比引入插件数据库表或每次匿名访问拉起插件子进程更轻量、更稳定。
