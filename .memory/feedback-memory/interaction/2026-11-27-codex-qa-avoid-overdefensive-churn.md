---
memory_type: feedback
feedback_category: interaction
topic: Codex 协作验收须在核心契约闭环后及时收口避免 xhigh 防御性内卷循环
summary: 当核心 AC、真实运行态测试与性能基线已验证通过后，Root agent 须自行甄别并拦下理论极端故障注入假设，禁止无上限拉起多轮 Codex QA 对抗与重复全量重测。
keywords:
  - codex
  - qa-evaluation
  - xhigh
  - over-defensive
  - stop-condition
match_when:
  - 使用 codex-commander 协作完成开发并进入 QA 验收或修复闭环阶段
  - 独立 QA 在主问题已解决后继续提出亚毫秒级手工改盘或瞬态故障注入假设
created_at: 2026-11-27 13
updated_at: 2026-11-27 13
last_verified_at: 2026-11-27 13
decision_policy: direct_reference
scope:
  - .agents/skills/codex-commander
  - .agents/skills/qa-evaluation
---

# Codex 协作验收须在核心契约闭环后及时收口避免 xhigh 防御性内卷循环

## 标题

核心问题与契约收据闭环后立即收口，禁止放任多轮 `xhigh` QA 陷入理论防御性内卷

## 时间

`2026-11-27 13`

## 规则

1. 指挥 Codex（`gpt-6.2-sol` / `xhigh`）执行 Single Issue 开发与验收时，常规闭环控制在 **1 轮集中独立 QA + 最多 1 轮真实 blocker 修复复核**。
2. 当用户目标对应的全部核心 AC、真实 PostgreSQL / 运行态测试、静态门禁与 A/B 指标均已跑通后，Root agent 必须亲自审查残余项：凡不属于“真实业务契约破坏、生产可达路径回归或架构红线违背”，而仅属于人为构造瞬态故障注入/手工改盘窗口的理论极端分支，一律按低风险残余边界说明收口，不再启动新一轮独立 Codex QA，也不再为局部小修重复捆绑全量 Clippy 与克隆库 A/B。

## 原因

两个 `gpt-6.2-sol (xhigh)` 会话在核心缺陷早已解决后，极易触发其固有的“防御性编程与局部细节过度优化”倾向，围绕低概率人工故障注入假设展开递减收益的攻防循环；每多跑一轮都会连带触发 25–35 分钟的编译、静态扫描与双阶段启动采样，严重拉长交付耗时且可能引入不必要的防御复杂度。

## 适用场景

- 通过 `codex-commander` 在本机或远程 `taichuy-dev` 指派开发任务与 QA 验收任务时。
- 根据 `qa-evaluation` 判断何时停止继续取证并向用户交付结论时。

## 备注

源自用户直接反馈：“问题搞定了，要qa这么多次吗”。后续必须在主契约与运行态证据充分时果断停止。
