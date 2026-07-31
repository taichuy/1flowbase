---
feedback_category: repository
decision_policy: direct_reference
created_at: 2026-04-28 17
updated_at: 2026-07-31 22
scope:
  - frontend
  - debugging
  - agent-flow
---

# 前端焦点类缺陷不要只补点击兜底

## 规则

当用户反馈前端焦点/输入框跳转问题“仍未修复”或质疑“是不是组件问题”时，必须暂停继续堆叠焦点兜底，先用最小回归测试和真实浏览器路径定位根因。

## 原因

焦点类问题常由 selection 变化、受控写回、contenteditable 子组件拦截事件、异步 blur 定时器共同触发。只看点击命中区域容易修到症状，不会覆盖“先点 A、再点 B、再输入”的真实序列。

## 适用场景

- React/Lexical/Monaco/Ant Design 等复合输入组件。
- 用户报告首次正常、切换字段后焦点跳回旧字段。
- 输入框内存在 chip、tag、mention、变量块等 contentEditable=false 子节点。

## 执行要求

先补能失败的回归用例，再用 Playwright 或项目 style-boundary 场景跑真实浏览器交互。验证至少覆盖：点击普通编辑区、点击内嵌 chip/tag、切回目标字段后直接输入。

Monaco completion provider 还必须按入口状态建立有限矩阵：触发字符查询、工具栏 / `Ctrl/Cmd + Space`、普通单词输入、候选选择替换和取消。只直接调用一次 provider 并覆盖其中一种上下文，不能结算真实编辑器补全；浏览器运行态未验证时不得把候选过滤、标签显示或 replacement range 宣称为已通过。
