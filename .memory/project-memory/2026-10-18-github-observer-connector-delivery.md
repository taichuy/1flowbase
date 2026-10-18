---
title: "GitHub 项目观察库与日周月榜（连接器配置）"
memory_type: project
created_at: "2026-10-18 01"
updated_at: "2026-10-18 23"
decision_policy: verify_before_decision
status: delivered
tags: [github, frontstage, connector, daily-report]
---

- 用户确认五张表及统一入口，由 Root 仅通过 1flowbase-gateway 连接器配置；不修改产品源码，不重启或破坏 7600。目的是将每日文档转为可检索项目历史并保留个人备注，未指定截止日期。
- 页面：`/news-highlights/pages/01a14aca-a722-71a5-9bdc-55f44d0585b4`，当前本机前端 `http://127.0.0.1:3400` 对接 7600；3100 对接 7800，不是本任务验收入口。
- 日榜区块：`01a14ae6-81d1-7574-84f0-96e3cdefc91a`；页签 `01a14aca-a722-71a5-9bdc-55f5ded6ba25`。2026-10-18 用户要求将五项内部导航放到左侧 github 分组下，已调整为五个独立页面，移除内容区重复导航；原日榜地址保留，数据服务与五张表继续复用。
- 表代码：`gh_projects`、`gh_observations`、`gh_daily_ranking`、`gh_weekly_ranking`、`gh_monthly_ranking`。仓库规范化标识唯一，材料按日期/来源/仓库去重；项目备注独立保存。
- 数据服务 Workflow：`01a14ad0-df0f-71e0-a0ac-58e1876a769b`，发布 `POST /api/ex/github-observer`；body 使用 `action` 与 JSON 字符串 `payload`。操作包括 daily/projects/materials/weekly/monthly/detail/save_project/upsert_materials/rebuild；复用前先核对当前契约。
- 定时 Workflow：`01a14ae2-4300-7769-a78d-13ec7982ac88`，`15 6 * * *`、`Asia/Shanghai`，已启用。只对已有材料幂等统计周/月榜，不自动外部采集；支持页面手动重算。
- 周月榜按同一来源榜单的有效上榜天数排序，缺少两个 Star 采集日时净变化为空；不累加 GitHub stars today。
- 已导入用户样例 2026-10-17 的 20 个项目（13 个官方榜、7 个社区精选）；保留全文材料与来源链接，7 个精选日增长为空。样例只作为用户提供材料，未重新在线核验事实。
- 验收证据：`tmp/test-governance/github-observer/qa/report.json`、`final/` 截图；浏览器验证备注保存重开与恢复、材料重复编辑不增行、五视图和手机布局。临时认证使用 page-debug owner 并回收。
- 恢复收尾已完成：`tmp/test-governance/github-observer/delivery-report.md` 为完整交付报告，`design-qa.md` 为视觉验收；`resume/regression.json` 12 项通过，控制台无警告/错误；初始手机视口及桌面缩到手机后的稳定布局均可到达第 10 行。
- 已回读真实定时运行 `01a14bee-cdf1-7582-85f1-b6df9e3f6a3b`：2026-10-18 06:15:00（北京时间）触发，约 5 秒 succeeded，周/月各 20 条。页面手动重算和定时 SQL 节点回归也通过。
- 恢复前后项目/材料/日榜整行 hash 一致，五表各 20，材料正文 20，测试备注残留 0；未重启 7600/7800、未改产品源码。当前仍只有单日样本，外部事实未重核，自动外部采集不在本次范围。
- 导航调整后的页面/页签/区块 ID 清单：`tmp/test-governance/github-observer/navigation/pages.json`；该目录 `report.md` 与 `qa.json` 记录 14 项真实浏览器验收通过（五入口、跳转、刷新、详情/备注读取、材料表单关闭、手机），控制台无错误；测试无业务数据写入。旧验收中的内部 Tab 导航步骤已被侧栏入口替代。
- 用户进一步要求三个榜单都按窗口高度与缩放收紧，避免大标题和高条目使内容向下延伸；已通过连接器统一日/周/月榜的可用高度计算与紧凑模式。日榜默认 Top10 首屏；周/月榜按实际行数分配表格行高，固定表头，长文字摘要+完整悬停提示，小窗口/展开全部时表格内滚动，手机自然滚动。周/月榜12个桌面场景及手机、详情/备注读取通过；日榜补跑原回归也通过。证据分别为 `tmp/test-governance/github-observer/height/report.md`、`period-height/report.md`，三榜最新修订见 `period-height/connector-receipts.json`。本次无业务数据写入、产品源码修改或服务重启。
- 2026-10-18 用户确认试用HTTP节点自动采集。已新增 Schedule Workflow“GitHub 观察 · 官方日榜采集” `01a14e94-c4f6-728a-8410-3feb04b47a42`，官方全语言 Trending 每天06:00 Asia/Shanghai，已发布并启用；06:15旧统计计划未改。当前发布版本 `01a14ea7-c99e-767d-89dd-6e6db9afb1ec`。下一次2026-10-19 06:00尚未发生，勿声称已定时触发。
- 首次真实HTTP整链成功写入2026-10-18官方13项：历史命中9、新项目4，现有24项目/33材料/33日榜；周月各24条（官方17、精选7）。同日真实快照重放新增0；历史20材料与20日榜整行hash、原20项目标题/备注/标签/关注状态hash一致。当前13项介绍中1项沿用历史。证据及全部边界见 `tmp/test-governance/github-observer/sync/delivery-report.md`、connector-evidence.json、browser-qa.json（7项浏览器验收通过）。
- 采集独立白名单SQL，不能用现有 `upsert_materials` 薄输入替代（它会清空分析）。保护所有分析/备注；同日更新事实，新日加快照；只撤当天退出的自动日榜关联，保留项目和材料。原始采集JSON及时间放source_document。空/坏页面和网络错误不写；无连续重试/补采/通知，真实试抓观察到网络失败并保留运行记录。README补充、社区自动采集及自动分析未接入，最新提交时间未由榜单提供则留空。本轮仍未改产品源码或重启服务。
- 2026-10-18 19 用户要求手动执行，已用同一Workflow整链调试入口真实重新请求GitHub（非快照重放），运行 `01a14eb8-e46c-71a0-9be1-7b6e764110e8` 28秒成功。榜单仍13项，新增项目3、材料4、退出自动日榜关联4；最新总数27项目/37材料/33日榜。原有保护字段及历史整行hash不变；示例Star42232→42247、Fork4500→4501。没有“立即触发调度”连接器入口，故不可将此次手动整链称作真实cron触发。
- 该真实成员变动暴露周/月派生表只upsert导致各1条孤立记录，已用连接器给统计SQL增加同语句scope限定的stale_w/stale_m清理，并同步数据服务rebuild；不删除材料/项目/日榜。真实数据复制到临时表7项回滚回归通过；统计整链 `01a14ebe-e734-74fd-9049-1dce3c280a81` 成功，周/月各26条、孤立0、名次唯一；浏览器发布rebuild再跑返回移除0，页面错误0。新发布：数据服务 `01a14ebe-a6a8-74b9-856f-01490ef27024`，统计 `01a14ebe-ccc6-7474-95c0-ba2a2b639822`。cron和采集配置未改、无产品源码变更或服务重启。最新证据 `tmp/test-governance/github-observer/manual-run/report.md`，此前数量以本条为准。
- 2026-10-18 20 用户纠正日榜应优先显示已有整理标题/介绍。根因是日榜读自动材料title，项目库中文标题其实保留。已通过连接器修改数据服务daily查询：title优先项目库，description优先已有整理材料再回退原文；增加项目库标题搜索。未改原始材料、采集/统计配置、样式或产品源码。数据服务最新发布 `01a14f11-1925-71a4-b64e-9cf5792a3398`。7项真实SQL回滚验证及发布后桌面/手机浏览器验收通过、错误0，五表整行hash前后一致。示例MadsLorentzen/ai-job-search现显示“AI Job Search——基于 Claude Code 的本地 AI 求职工作流”；新项目保留原始英文简介。证据 `tmp/test-governance/github-observer/curated-copy/report.md`。
- 2026-10-18 22 用户确认图片导出先做日榜、默认Top10，已通过连接器给原日榜区块加“导出图片”按钮。独立海报Canvas→PNG，当前3840×2224；按所选日期/来源/语言/关键词强制page1/show_all=false读取daily，展开全部不影响导出范围。沿用整理标题与真实数值，简介最多2行，未披露不填0；项目图标为所属者缩写徽标。原始数据、备注、产品源码与服务未动。当前区块SHA `c0cb70808e6ecd75869550427e53b2a14ec6ef3f1d4ab4864f1a154d9fffb104`，替代此前54a48...修订。9组真实浏览器验收通过，包括6次PNG下载、历史日期/关键词/展开全部/空结果/手机/社区未披露/故障恢复，无业务写入或pageerror。证据 `tmp/test-governance/github-observer/image-export/report.md`、qa.json，成品 `artifacts/daily-top10.png`。周/月图片导出未扩展。

- 2026-10-18 23 用户确认社区独立类目，已通过连接器拆出第六个页面「社区精选」`01a14f87-5f8b-7073-b92e-f6573f7abe52`（github下日榜后），区块`01a14f8c-2c36-77f2-980d-4abd0d2e6456`，修订`d38e3857ef22541bdef5104f0db77ef5868d6f5ebe0391c5dfd5d1fc112cf615`。默认最近收录、来源/关键词/收录日期范围筛选，每项目一条最近匹配记录；无排名或增长徽标，展示历史反馈、原讨论与收录日期，事实放项目资料；备注复用项目库。日/周/月页面不再提供社区切换。官网/Apple研究文章单列补充参考，不算社区讨论来源。社区导出最近最多10项PNG（不称Top10）；现有7项与筛选单项/手机实际下载均通过。数据服务新增只读community action，后端SQL分类/筛选/去重/排序；最新发布`01a14f97-3ea3-773b-b73c-79b607c639b2`，原mapping保留。12组最终QA通过、页面错误0，额外手机直接进入/最后一条/页脚/抽屉3项通过；四次真实PNG下载。五业务表全行hash与行数27/37/33/26/26前后一致，旧社区关联保留。未改采集与统计定时任务、未增加社区自动采集、未改产品源码或重启服务。交付与证据：`tmp/test-governance/github-observer/community-split/report.md`；页面主逻辑快照在snapshots/community.jsx。无新增截止日期，本轮已经交付。

- 2026-10-18 23 用户要求社区列表改为基础信息紧凑表格，已用连接器调整同一社区区块（修订`7be646ef90a70379bf3c60ed3aabfd7998a4c64ce48c3938cb2a6811cc4cea60`）：仓库/标题/Star/来源/收录时间/描述六列，每行42px，描述和标题单行省略，完整分析与备注继续在详情；Star为收录快照stars_total。小屏内部横向滚动保留六列。7组真实浏览器验收通过，0页面错误，筛选/详情/备注读取/PNG下载正常，无业务写入。只改社区UI配置，无工作流/数据/进程变更。最新样式证据`tmp/test-governance/github-observer/community-compact/report.md`；此前community-split宽摘要视觉已被本次取代。

- 2026-10-18 23 社区表格累计Star按用户要求改为>=1000的一位小数k格式，悬停完整数字与收录日期；同一社区区块最新修订`5357c0359a412d48a4de7090135eb9f913cbcebd07f6570966a9235aa3ccfd50`。七项真实页面数据/显示/提示及42px行高断言通过，页面错误0；证据`tmp/test-governance/github-observer/community-stars/qa.json`。仅UI配置改动。
