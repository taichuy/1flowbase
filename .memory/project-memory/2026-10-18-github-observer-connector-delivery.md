---
title: "GitHub 项目观察库与日周月榜（连接器配置）"
memory_type: project
created_at: "2026-10-18 01"
updated_at: "2026-10-18 12"
decision_policy: verify_before_decision
status: delivered
tags: [github, frontstage, connector, daily-report]
---

- 用户确认五张表及统一入口，由 Root 仅通过 1flowbase-gateway 连接器配置；不修改产品源码，不重启或破坏 7600。目的是将每日文档转为可检索项目历史并保留个人备注，未指定截止日期。
- 页面：`/news-highlights/pages/01a14aca-a722-71a5-9bdc-55f44d0585b4`，当前本机前端 `http://127.0.0.1:3400` 对接 7600；3100 对接 7800，不是本任务验收入口。
- 区块：`01a14ae6-81d1-7574-84f0-96e3cdefc91a`；页签 `01a14aca-a722-71a5-9bdc-55f5ded6ba25`。一个 React 配置区块包含日榜、项目库、周榜、月榜、原始材料五个视图；日榜按用户附图实现。
- 表代码：`gh_projects`、`gh_observations`、`gh_daily_ranking`、`gh_weekly_ranking`、`gh_monthly_ranking`。仓库规范化标识唯一，材料按日期/来源/仓库去重；项目备注独立保存。
- 数据服务 Workflow：`01a14ad0-df0f-71e0-a0ac-58e1876a769b`，发布 `POST /api/ex/github-observer`；body 使用 `action` 与 JSON 字符串 `payload`。操作包括 daily/projects/materials/weekly/monthly/detail/save_project/upsert_materials/rebuild；复用前先核对当前契约。
- 定时 Workflow：`01a14ae2-4300-7769-a78d-13ec7982ac88`，`15 6 * * *`、`Asia/Shanghai`，已启用。只对已有材料幂等统计周/月榜，不自动外部采集；支持页面手动重算。
- 周月榜按同一来源榜单的有效上榜天数排序，缺少两个 Star 采集日时净变化为空；不累加 GitHub stars today。
- 已导入用户样例 2026-10-17 的 20 个项目（13 个官方榜、7 个社区精选）；保留全文材料与来源链接，7 个精选日增长为空。样例只作为用户提供材料，未重新在线核验事实。
- 验收证据：`tmp/test-governance/github-observer/qa/report.json`、`final/` 截图；浏览器验证备注保存重开与恢复、材料重复编辑不增行、五视图和手机布局。临时认证使用 page-debug owner 并回收。
- 恢复收尾已完成：`tmp/test-governance/github-observer/delivery-report.md` 为完整交付报告，`design-qa.md` 为视觉验收；`resume/regression.json` 12 项通过，控制台无警告/错误；初始手机视口及桌面缩到手机后的稳定布局均可到达第 10 行。
- 已回读真实定时运行 `01a14bee-cdf1-7582-85f1-b6df9e3f6a3b`：2026-10-18 06:15:00（北京时间）触发，约 5 秒 succeeded，周/月各 20 条。页面手动重算和定时 SQL 节点回归也通过。
- 恢复前后项目/材料/日榜整行 hash 一致，五表各 20，材料正文 20，测试备注残留 0；未重启 7600/7800、未改产品源码。当前仍只有单日样本，外部事实未重核，自动外部采集不在本次范围。
