---
title: "供应商插件由用户手动下载安装"
memory_type: feedback
feedback_category: repository
created_at: "2026-09-10 15"
updated_at: "2026-09-10 15"
decision_policy: direct_reference
status: active
---

- 规则：供应商插件由用户自行手动下载安装，不应把现有镜像下载和启动自动安装逻辑当作已认可需求。
- 原因：用户在本地镜像下载失败诊断中明确质疑自动下载供应商插件，纠正了只修代理、保留自动打包的建议。
- 适用场景：供应商插件安装入口、镜像打包和启动 bootstrap 的诊断与修改。此纠正本身不表示已完成代码移除。
