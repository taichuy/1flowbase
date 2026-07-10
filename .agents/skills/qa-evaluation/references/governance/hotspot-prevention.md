# Hotspot Prevention Review

## When To Use

用户要求分析昨天/今天、近两天、近期代码热点、反复修改、AI 编程效率或 Hermes agent 开发环境优化时使用。

## Goal

把高频修改转化为 AI 下次开发前的判断规则。默认输出 prevention layer，不停在业务代码修复建议。

## Minimum Evidence

- `git log`：确认时间窗口、提交主题和提交数量
- `git log --name-only` 或 `git numstat`：找出高频文件和高 churn 模块
- 优先运行 `node scripts/node/cli/hotspot-review.js --since "2 days ago"`，报告产物固定写入 `tmp/test-governance/hotspot-review.json`
- 当前源码：抽样确认热点是否仍存在结构压力
- 相关 `.memory` 与现有 `.agents/skills`：判断是新规则缺失，还是已有规则未触发

## Classification

| 热点类型 | 典型信号 | 应沉淀到哪里 |
| --- | --- | --- |
| UI 信息架构 churn | 同一页面反复移动入口、标题、卡片、按钮、modal | `frontend-development` / `frontend-logic-design` |
| 运行态真值 churn | cache、last run、snapshot、latest、run detail 多入口互相补丁 | `backend-development` state consistency；必要时补 QA gate |
| 质量门禁 churn | lint、format、clippy、CI、coverage 反复补救 | `qa-evaluation` 与 `scripts/node` |
| 目录 / 文件压力 | 高频文件接近或超过 1200/1500 行，目录横向膨胀 | AGENTS directory rules 或新增 size/churn report |

## Output Shape

必须包含：

- 热点事实：哪些文件、几次、哪些提交主题
- 自动报告：`tmp/test-governance/hotspot-review.json` 的核心发现；如果未运行必须说明原因
- 归因：缺少哪类前置判断，而不是只说“代码复杂”
- Skill 更新建议：具体到 skill 文件或 reference 文件
- 环境更新建议：具体到 AGENTS、脚本、质量门禁或检查命令
- 风险收益：更新规则的收益、误伤风险和停止条件

## Stop Conditions

- 已能指出 1 到 3 个最该更新的 skill / rule / gate
- 已能解释这些更新如何减少下一次 AI 返工
- 不继续扩展成全项目代码重构建议，除非用户明确要求
