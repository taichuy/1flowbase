---
memory_type: feedback
feedback_category: interaction
topic: blocked-tool-no-subagent-detour
summary: 当 Bash 等执行工具被 harness 权限分类器或环境故障阻塞时，不要用子 agent 绕路；直接重试或明确汇报阻塞点并给出用户可一键执行的命令。
keywords:
  - subagent
  - 子模型
  - Bash
  - permission-classifier
  - auto-mode
  - blocked
match_when:
  - Bash / CronCreate / Agent 返回"分类器临时不可用"或类似环境故障
  - 想通过 Agent / 子 agent 代替自己执行被阻塞的命令
  - 初始化、同步、提交等纯执行类任务被工具层卡住
created_at: 2026-09-17
updated_at: 2026-09-17
last_verified_at: 2026-09-17
decision_policy: direct_reference
scope:
  - Bash
  - Agent
  - CronCreate
---

# Blocked Tool: No Subagent Detour

## 规则

执行工具被环境故障阻塞时，由当前 agent 直接重试；不要为绕开阻塞而调用子 agent / 子模型。重试无效时，停止并向用户汇报：已完成什么、卡在哪一步、用户可用 `! <command>` 直接执行的命令。

## 原因

用户在 2026-09-17 明确纠正："不要调用子模型，直接执行"。子 agent 与主 agent 共用同一权限分类器，绕路无效，只会额外消耗 token 并让用户失去对执行路径的感知。

## 适用场景

- `.agents/skills` → `.claude/skills` 同步、测试运行、提交等纯执行任务被 Bash 分类器故障卡住。
- 任何"工具不可用"场景下想用 Agent 顶替执行。
- 汇报时优先给可一键复制的命令，而不是解释重试过程。
