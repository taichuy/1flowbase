---
date: 2026-09-11 15
feedback_category: interaction
decision_policy: direct_reference
---

# Gateway Acceptance Must Cover Active Installation

- 规则：用户要求网关正常持续工作时，临时服务上的单次工具循环仅作为候选证据；验收还须绑定实际启用的宿主、插件、传输配置，并覆盖重启后的新会话和多轮工具交互。
- 原因：用户在候选通过后重启常用服务仍遇到只输出行动说明而不执行，暴露候选组合与实际安装组合不同；不能把临时探针成功表述为用户使用路径已完成。
- 适用场景：1flowbase Responses 网关、配套插件升级与 Codex 持续工具任务。依据接口生命周期架构检查正式协议输出、调用终态与交付证据，区分部署不匹配和协议设计缺口。
