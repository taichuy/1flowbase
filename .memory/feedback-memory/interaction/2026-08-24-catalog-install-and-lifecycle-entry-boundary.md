---
memory_type: feedback
feedback_category: interaction
topic: 在线目录只负责目标版本安装，本地列表拥有卸载与清理
summary: 用户确认在线插件目录的卡片应按目录目标版本判断能否安装，不因家族存在旧版本而显示已安装；卸载、历史版本清理与切换统一留在左侧本地安装列表。
keywords:
  - plugin catalog
  - install status
  - uninstall
  - version family
  - network center
match_when:
  - 设计在线插件目录与本地已安装版本列表
  - 判断安装、更新、版本切换、卸载或历史版本清理的入口归属
created_at: 2026-08-24 11
updated_at: 2026-08-24 11
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/network-center/providers
  - api/apps/api-server/src/routes/network_center/plugins.rs
---

# 在线目录与本地生命周期入口边界

## 规则

- 右侧在线目录卡片代表目录提供的具体目标版本；只有该目标版本真实安装后才显示“已安装”，存在同家族旧版本不能阻止安装。
- 右侧目录只保留安装能力和目录信息，不展示卸载、旧版本清理或版本切换。
- 左侧本地安装列表拥有当前版本、版本切换、更新提示、卸载和必要清理动作；同一生命周期动作不在两侧重复暴露。

## 原因

在线目录拥有“有哪些版本可获取”的真值，本地列表拥有“当前安装了什么以及如何管理”的真值。把家族级安装状态投影到具体目录版本会造成假“已安装”，把卸载同时放在目录卡片和本地列表会造成入口重复与职责混乱。

## 适用场景

Network Center、Extension Center、模型供应商等同时展示在线目录与本地已安装对象的页面。
