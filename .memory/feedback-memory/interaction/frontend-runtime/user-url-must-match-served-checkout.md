---
title: "页面交付必须核对用户实际 URL 的源码归属"
memory_type: feedback
feedback_category: interaction
created_at: "2026-11-11 21"
updated_at: "2026-11-11 21"
decision_policy: direct_reference
status: active
tags: [frontend, worktree, runtime, user-url]
---

- 规则：交付用户指定页面或宣称合并后已生效时，核对用户实际端口的进程工作目录、实际返回模块与浏览器页面；隔离端口截图只能证明隔离候选，不能证明用户当前入口已更新。进程参数中的 Vite 可执行文件路径和主仓 PID 文件也不能代替进程 cwd。
- 原因：本次 MCP 表格在隔离 3191 验证并合入 dev 后，用户 3100 仍显示旧表格。3100 进程参数指向主仓 node_modules，但 /proc/PID/cwd 实为 chat-command-events 旧工作树，返回模块没有 DataTable。仅查看 dev 源码会得出错误结论。
- 已验证处理：在主仓使用 dev-up restart --frontend-only --skip-docker，使 3100 实际 cwd 回到主仓 web/app；随后在真实 3100 用临时 session 和 Playwright 检查两个 MCP 页签，确认列设置、分页可见，7800 / 7600 后端 PID 未变。证据：/home/taichuy/.codex/worktrees/ssh-upgrade-activation/1flowbase/tmp/test-governance/mcp-tables/main-3100-evidence.json。
- 适用场景：多工作树、多前端进程、端口复用，以及用户页面与 Agent 截图或源码不一致。旧工作树有无未提交改动须核查并保留，不通过改写另一个任务的源码解决入口不一致。
