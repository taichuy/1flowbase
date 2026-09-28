---
title: Respect latest runtime port ownership correction
date: 2026-09-28 16
summary: 生命周期任务中用户更正为保护7600、允许重启7800；按最新授权核对运行进程和启动脚本的PID记录。
feedback_category: repository
decision_policy: direct_reference
---

规则：用户更正端口授权后，旧计划中的相反授权立即失效。执行重启或故障注入前，同时核对目标端口、父进程身份及启动脚本持有的PID记录；明确保护的进程不得被旧receipt误选。

原因：同一可执行文件、同一worktree可同时服务多个端口，仅凭路径或旧PID记录不足以判断授权范围。

适用场景：有多个本地API服务的生命周期测试、重启与子进程故障注入。当前#2153保护7600，7800为允许重启的验证服务；这是本任务边界，不将两个端口固化为所有任务的全局规则。
