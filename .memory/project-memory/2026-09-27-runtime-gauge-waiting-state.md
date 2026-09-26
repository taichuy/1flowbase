---
memory_type: project
topic: runtime-gauge-waiting-state
summary: 用户确认系统运行页环形指标缺失新样本时用小型动画替代圆环内文字；持续不可用时圆环内用破折号、圆环外说明状态。
keywords:
  - system-runtime
  - gauge
  - loading
match_when:
  - 调整系统运行页的指标预热、过期、不可用展示
created_at: 2026-09-27 07
updated_at: 2026-09-27 07
last_verified_at: 2026-09-27 07
decision_policy: verify_before_decision
scope:
  - web/app/src/features/settings/components/SystemRuntimePanel.tsx
---

# 环形指标等待态

用户发现系统运行页 CPU 环形指标会短暂在圆环内显示并折行“等待新样本”，认为不好。用户确认平衡方向：预热或样本过期且没有数值时使用与通用加载动画一致的小型动画；持续不可用时在圆环内显示 `—`，状态说明放在圆环外；有数值时继续显示百分比。这样既消除文字闪现和换行，又不把长期不可用误报为加载中。本轮由 AI 实现，没有固定截止日期。
