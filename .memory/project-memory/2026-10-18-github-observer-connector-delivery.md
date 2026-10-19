---
title: "GitHub 项目观察库与日周月榜（连接器配置）"
memory_type: project
created_at: "2026-10-18 01"
updated_at: "2026-10-19 16"
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

- 2026-10-19 00 用户休息前明确要求新增报告页，参考route日报页面。已通过连接器交付github→报告：页面`01a14fc1-a65f-75ee-9c92-a7c891b6b286`、区块`01a14fc5-6e64-7690-90b9-b2c99228d51a`，新表gh_reports模型`01a14fbf-1e65-7315-b183-ac26431e17aa`；统一日报/周报/月报/其他，标题/周期/摘要/Markdown正文，新增/原记录修改/阅读/搜索/筛选/确认删除/Markdown导出。使用标准已认证动态CRUD，无新Workflow或调度。可视化与Markdown编辑预览，未保存确认、失败保留草稿；修复Vditor格式规范化误标记dirty，切换/保存前读取当前草稿。最终修订`ecdfd80022ab9e83aaca6e0b366d97972bdcdbd5583a461566a3a7d9c8bc0dce`。10组真实QA通过、0页面错误，14,364字符正文持久化，手机可读可编辑，测试记录全清理，新表0条；原五表及daily_reports15条全行hash保持一致。未改产品源码或重启服务。新页不使用旧榜单视口自算；旧日榜高度仍仅诊断，未扩大本轮范围。交付报告`tmp/test-governance/github-observer/report-library/report.md`。

- 2026-10-19 07 用户纠正报告快速预览应为标题列表、标题检索、时间正反排序，并要求两个真正独立且可调宽的原生区块。已通过连接器将原报告区块改为目录，新增正文区块 `01a15159-a3e5-705e-a361-7570ea6455bc`；原页面和gh_reports表保留。桌面7/24+17/24，手机上下排列/点击阅读抽屉；高度直接消费平台available.height，一次性固有偏好560。选择与CRUD后刷新经端口联动。目录最终SHA `fa01377682f68c68e1ab6ffb30cd92a64003f9667288b3c58844207074a4bf5e`；正文SHA `e590691bd2f0a0ffdee09d7bee1bc2800bdd93ca400f26edab1c1446cea14064`。10组真实浏览器验收通过、0页面异常；35篇滚动/搜索/正反序和单篇CRUD测试数据全部移除。7表完整行hash前后一致，当前数量gh_reports0、projects27、observations50、daily46、weekly39、monthly26、参考daily_reports15。设计模式两块各有原生宽度拖拽手柄；未在UI中保存拖拽布局，实际拖拽持久化未另测。无产品源码、其他榜单、工作流/调度或进程变更。证据 `tmp/test-governance/github-observer/report-navigator/report.md`。此前report-library单区块布局已被本次替代，旧编辑器保护继续复用；未指定截止日期。

- 2026-10-19 15 用户确认周/月榜上期比较与排名历史方案，已仅通过连接器交付：主榜增加上周/上月排名及升降，详情展示跨缺席周期的上次上榜；无上一完整周期记录显示无对比数据，保持原observed_days+project_id排序。新增表gh_ranking_history（模型 `01a15279-f156-75e4-bdc8-81d66880cbf0`）及github→排名历史页面 `01a152eb-a01b-70fd-a8c2-d0afc6dd0fdf`，区块 `01a152eb-b83f-769a-b193-ca688ca1d8a4`。项目/类型/快照日期筛选、分页、全榜和单项目历史抽屉均可用。新快照只记录当前周期官方榜、按北京时间统计日幂等；同日退出清理，旧日期保留，和统计单语句原子写入；不伪造启用前快照。首次真实统计2026-10-19共32条（week13/month19）；实际下次cron未等待。原06:15 cron/timezone/enabled/input_payload和06:00采集未改。数据服务发布版本 `01a152e8-7cbc-7272-a960-9a0be3e13d69`；统计版本 `01a152e8-8ec0-778b-a2e2-b9b8b04cfec7`。周/月区块最终SHA分别 `001a333388d13f23b269a6382445a365ab4cef0601e9ffafc4beef41c3f14ad2`、`0f5a0c85283fe923692aa5a8b0a2337652536b3564dfea445b668f67f49d21a1`。11项事务回滚检查、8组浏览器/API和手机真实横滚补验通过，页面异常0。原项目35/材料60/日榜56/参考日报15/用户报告1全行hash不变；周/月派生因重算纳入已有社区材料变49/34，官方榜仍本周13/月19，原排序SQL对照差异0。无业务测试残留、产品源码改动或进程重启。交付与证据 `tmp/test-governance/github-observer/rank-history/report.md`、manifest.json、qa.json。未指定截止日期，当前任务已交付；现阶段依然缺真实上月数据。

- 2026-10-19 15 用户否定当前日/周/月/社区/定时任务的整体组织，进入结构重审。本轮仅只读核对：采集仍06:00，统计06:15独立；日榜是官方Daily，周/月是observed_days+project_id派生；同频次内部ID解平手不代表GitHub热度。已询问周/月最终采用官方榜、自己统计或明确分开两者，尚待答复；推荐统一官方趋势入口，但该方向未获批准，不应实施或当作已确认。原数据/页面/任务全部保留，无停用或重构。此前局部QA有效但不能据此认定整体产品合理。

- 2026-10-19 15 用户已回复口径选择：周/月必须是“按每日收录统计的周/月观察榜”，不切换为GitHub官方Weekly/Monthly Trending。上一条推荐官方榜的方向不采用。用户随后明确确认整体结构和规则调整，已进入并完成连接器配置，以下最新状态取代上文旧结构/调度口径。

- 2026-10-19 16 已完成整体整理：GitHub下四个业务入口“项目发现”（原日榜地址，内切官方日榜/社区精选）、“观察统计”（原周榜地址，内切本周/本月）、项目库、报告。原始材料/排名历史和旧社区/月榜地址移入“管理与存档”分组`01a1533f-e5b0-7196-83f5-c8e336ce7702`，稳定页面/区块ID均保留，无新业务表。周/月只计官方日榜项目、每项目每天一次，使用competition rank同天数并列；覆盖日期/天数、周期进行状态、统计成功时间均由数据服务返回。社区不参与统计；报告仍为两个独立人工编辑区块。
- 唯一统计owner为通过连接器安装的数据库函数`public.gh_rebuild_observation_statistics_v2()`；采集工作流增加保存成功后的SQL节点，原统计应用和页面rebuild都调用该函数。06:00采集cron保持启用，06:15独立统计cron已停用；管理抽屉支持仅重试统计。快照日期取实际最新收录日，旧32条快照hash不变，新口径snapshot_key带observed-days-v2前缀并明确标注，跨口径不比较。数据服务最新发布`01a15346-8892-7357-84e9-507c0d19f412`，采集`01a15329-2752-75cd-a573-6565188bb7fa`，保留统计应用`01a15329-1680-77b2-900e-1925ac68f38e`。运行状态按当前publication.document_hash过滤，排除旧版本/临时故障文档。
- 真实HTTP整链`01a1532a-e381-7549-9ed5-937ae9673995`成功：官方14项、历史命中13、新项目/新材料各1，随后周/月统计和34条新快照。当前36项目/61材料/57日榜/50周统计/35月统计/66历史，用户报告1和参考日报15不变。旧32历史、用户报告与参考日报完整行hash均保持。临时debug文档受控解析失败`01a1532e-5061-779d-b2bc-6dadc500d859`只执行start/HTTP/失败解析，保存与统计未执行，统计及快照hash不变，正式草稿未受影响。
- QA：9项事务回滚测试全过，旧实现能在并列/社区两项被拒绝；补验7组全过，含真实Top10 PNG下载、手机第10项/社区最后项/周月最后行滚动可达、实际导航顺序、独立历史页与管理文案。手机内容曾超出宿主分配导致最后条目不可达，已通过消费available.height及内部滚动修正。主要8组浏览器/API回归与最终证据见`tmp/test-governance/github-observer/coherent-model/report.md`、qa/connector-evidence.json、qa/browser.json、qa/final.json。新cron下一次2026-10-20 06:00未等待，不称已定时触发。
- 为完成真实导航移动，按MCP配置skill补充`frontstage_move_page`到当前1flowbase实例/frontstage（Tool record`01a1533e-f47c-7133-93a0-35fb7a9bb0d1`，binding`01a1533f-063b-7393-b321-682a4a6c1aa6`），复用既有move_frontstage_page接口，list/get/call和循环父子400拒绝通过。未改产品源码、认证/业务接口或重启任何服务。
- 最终手机验证额外覆盖“等待分配稳定后，行与全部裁切祖先的可见交集”，避免仅凭行坐标在视口内而误判通过；移动端固定上报固有偏好、实际高度消费宿主allocation，不将allocated height反向上报造成收缩。最终主要区块SHA：发现页`e72aeaca27cedca0dcb75d3130522bac396a982c951457a8e61163f9f3317d2a`、观察统计`ae0473debbd3c7d4dc782f2a6e211e841352f29019eb108252154e44c7493250`、保留月榜地址`2b525fa5b4dad4d73f0268f6705d7b485d503703f07033ed629c6cf8ad831c57`。当前社区主入口已嵌入发现页区块；独立社区旧地址仅归档保留，后续配置应优先修改发现页中的社区组件。
