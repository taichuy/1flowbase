---
title: "角色自动接收新增后台权限的确认边界"
memory_type: project
created_at: "2026-09-10 18"
updated_at: "2026-09-10 19"
decision_policy: verify_before_decision
status: active
tags: [roles, console-policy, auto-grant]
---

- 用户于 2026-09-10 确认 Issue #2024 的平衡方案，由当前 Root 完成后端修复。
- 决策：自动接收覆盖未来新增后台接口与分组；初次建立目录基线时不补授历史缺项。关闭开关、关闭分组及手动撤权必须保留，重启和插件重新出现不能恢复旧授权。
- 新分组采用 custom 明确记录授予项，使之后关闭自动接收时停止未来新增授权；已有 full 分组保持其原有完全访问语义。
- 动机：界面开关此前只接入旧权限和动态页面，后台新接口需要逐项补配；修复应履行“仅未来新增”的承诺，同时避免覆盖管理员配置。
- 范围与期限：当前开发环境与本轮 Issue #2024；交付后以该 Issue、当前代码及运行日志为事实来源，不将本记录视为额外授权。
- 计划入口：https://github.com/taichuy/1flowbase/issues/2024
