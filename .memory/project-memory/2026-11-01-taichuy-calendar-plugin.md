---
memory_type: project
topic: taichuy-calendar-plugin
summary: 日历插件0.1.19本地已注册完整可编辑区块模板并同步最新引语；六组件保持独立注册，旧手工模板4修订备份后删除；正式发布与云端部署分别核验。
created_at: 2026-11-01 09
updated_at: 2026-11-04 22
decision_policy: verify_before_decision
scope: [runtime-extensions, frontstage, ui-management]
---

## Approved scope
用户于2026-11-01确认：新建独立插件，五个功能组件（月历、黄历、出生信息、紫微斗数、八字）及完整联动组件；完整复刻 /bazi 样式交互；准许插件内引入 lunar-typescript 1.8.6、tyme4ts 1.3.9、iztro 2.5.3、astronomy-engine 2.1.19、china-division 2.7.0。目标页见 reference-memory/2026-11-01-taichuy-bazi-source.md；为完整迁移并避免破坏底座职责，不把业务混入 browser-obscura。

## Active control
唯一计划真值为 taichuy/1flowbase Issue #2758。无承诺交付日期。
Root当前已完成只读Scout和源站固定输入取证，未进行产品改动、安装或发布。2026-11-01的核对显示：v2支持block_contributions及frontend module资产，安装投影到Block目录；组件管理另读ui_component_records，尚无plugin-owned组件记录及其生命周期投影。该证据使用前须复核当前源码，不应误报为“不支持前端插件”。

## Decision gate
此前用户批准方案明确约定：若缺底座必要注册契约，先报告，不擅自扩大底座contract，不直接写库或创建Custom记录冒充自动注册。用户随后回复“允许，确定”，已批准补齐通用的插件组件注册及升级/停用/卸载生命周期，停止条件解除；禁止业务特判与Custom记录绕过。原插件UI迁移目标不变。

## Local boundary
插件assembly复用了已检查clean、无进程和近期写入的 /home/taichuy/git/git_worktree/pricing-publisher-20261031，分支codex/taichuy-calendar-20261101，自ff765cc起；原pricing分支保留。主仓dev及插件main未切分支。该目录名不代表本轮任务内容；继续前核实HEAD/dirty/活动进程。

## 2026-11-01 09 Build Batch
宿主隔离assembly `/home/taichuy/git/git_worktree/taichuy-calendar-host-20261101`，branch `codex/taichuy-calendar-host-20261101` baseline `41099231a`。H1 api owner Dalton，P1 新插件目录 owner Arendt，W1 web owner Root。唯一root #2758；Test Batch及写集在该Issue及tmp/test-governance/taichuy-calendar/root-issue.md。当前未安装、未推送、未发布。

## 2026-11-01 11 当前修复状态
已完成代码及本地插件包；QA1报告B1启用策略拒绝、B2/B3factory依赖owner边界、B4publisher ownership、B5真实安装fixture缺managed bindings。完整证据及命令在tmp/test-governance/taichuy-calendar/qa1。WF1宿主ce15e91d7和PF1插件27d6a079已提交；HF1出现两个API writer未提交重叠，均已停止，Root保留备份并统一使用publisher/artifact canonical owner及单份migration。仍未合dev/push/cloud，原目标页面未装配。Root候选进程API7812/web3112，不改原7800/3100。主dev推进至011146374且有其他任务dirty，不得覆盖。下一步冻结修复候选并fresh QA2；source/hash未变的绿项复用。

## 2026-11-01 11 Latest decision state (supersedes historical progress above)
QA2为QA_FAIL：service fixture缺managed archive identity，幂等/升级断言未执行；已知Studio时序warning再次失败。合法包assign→enable已成功、六组件记录可用，MCP目标页已装配，但仅候选3112/7812，未完成真实installed页面独立验收、未切换用户3100。host b2cdf92cc与plugin27d6a079 clean；主dev011146374其他任务dirty原样保留。Root停止继续修复/QA循环并收敛最小方案：保留产品契约，fixture使用标准归档入口，Studio等待确定化，版本/包一致后集中验收。未合dev/push/cloud。唯一控制真值Issue2758及tmp/test-governance/taichuy-calendar/reframe-after-qa2.md；继续前须核对用户决策和当前运行态。

## 2026-11-01 11 Reframe resolved / QA3 (supersedes STOP entry)
QA2最终明确：QA1缺managed bindings与QA2缺归档身份是两个独立前置，不是同根因二次失败。Root在原授权内仅修fixture：真实tar→install_uploaded_plugin，不造checksum；Studio等待真实compiler/shadowRoot，保留并加强断言。host d86268ffb（产品代码等同已编译b2cdf92cc）；plugin a015456，版本0.1.1，完整包/README/manifest一致，标准upload/assign/enable成功。唯一fresh QA3 Raman01a19534-933e-7883-a48a-3aa2f9d4d950，冻结候选不可改/不得并发lifecycle，原3100/7800不动。允许独立项完成一次集中取证，不因无关fixture失败提前跳过live；同根因再次失败停止，不再fix循环。主仓无关dirty仍保留，未合dev/push/cloud。

## 2026-11-01 13 Latest state (supersedes QA3-era entries)
已关闭QA3/4/5/6，保留所有失败。QA3页面descriptor缺identity已MCP修正；QA4 fetched间接antd-style未启surface owner，host95afe9e226修正并QA5/6双端通过；QA5 literal.ant前缀fixture与产品CSS适配分离，插件8ae1094修CSSprefix，5d18f0d保留originNode class但首次TS2322，5ca4898类型修正后0.1.4，JS hash与0.1.3不变514a8b8...。QA6双端农历popup/尺寸/prefix/layout/refresh、ZiWei day/limit/year/month/宫位/copy通过；desktoptime/place/gender/solar通过。剩余fixture点第二大运后查第一大运2034，已明确是fixture不是算法错误；Root修为activepanel10个consecutiveyearkeys选首个。当前唯一QA7 Lorentz01a19599-f2cf-7db3-83b7-522252d8c2bd。
冻结host95afe9e226/plugin5ca4898da149ab8d388251278f79bc95cb042cf8 clean；candidate3112/API7812 binaryb2cdf92cc后端未变。0.1.4标准upload/assign/enable成功install01a19597-ff7d-71c2-b2de-5d0ae6e46df6，MCP目标block版本同步。主dev011146374其他任务dirty保留；7文件hash/patch备份在integration-preflight；主7800PID1314155及3100PID3837026未动。等待PASS再主环境cutover；既有live disable/uninstall409保留未验证，不再重试。
插件远端main7501024仅157个pricing文件较本地主mainff765cc推进，不能覆盖。云端发布需配套宿主契约部署；旧main0.6.1无支持，README已明确，不能将候选源码push冒充正式兼容上线。Root2758 ledger及qa7证据为后续入口。

## 2026-11-01 14 VERIFIED_LOCAL / PUSHED_SOURCE (supersedes QA7 waiting state)
QA8有限候选PASS已关闭，Root主环境smoke9checks通过：实际3100双端native挂载/图表切换/刷新、设置6条plugin组件、7800页面descriptor/source回读，console/pageerror0。宿主主dev已FF到95afe9e226并push远端dev；7个无关文件hash完全保留，主dirty未提交。插件主main先FF远端pricing7501024再合并5ca4898，HEAD a2a51cb9f3fce12c89223abe6c192323daaaa573 clean并push远端main；0.1.4目录等同候选，真实包8payload一致。主节点api-node-4fdf009b98c0标准upload201/assign200/enable200通过，install仍01a19597-ff7d-71c2-b2de-5d0ae6e46df6。主7800PID1573775验收binary，3100原Vite3837026未重启。候选3112/7812及wrapper已全关闭；临时认证清理；候选worktrees保留无活动进程。
正式签名发布/云端部署未完成：现provider-release检测UI-only空slot返回include[]，host远端main223e1aac旧契约不支持，需配套host rollout/发布工具支持；源码push≠发布部署。Root2758及releaseDelivery保持pending。Q8-W01复制地点描述latitude可能旧值保留warning，OSclipboard/live成功disable-uninstall/全像素/云端未验。完整交付tmp/test-governance/taichuy-calendar/main-integration/final-event.md与smoke-main.json，勿再启动QA9或把已保留失败抹绿。

## 2026-11-01 15 Follow-up decision (supersedes registration ambiguity above)
用户明确数据问题先归插件注册规范，列表滚动归底座；日历插件只注册到组件管理，不能用两个旧投影合理化重复入口。此修正保留代码模板独立用途、已有人工模板和组件；不合并或删除管理页签，不新增第三方依赖。本地实际入口已完成接口、桌面/手机及短视口滚轮验收；注册契约见 `api/plugins/frontend-ui-registration.md`。正式签名包发布与云端部署仍不等同于源码推送，无承诺完成日期。旧阶段进度只作历史，不作为继续执行状态。

## 2026-11-03 23 Plugin policy registration decision
用户已批准 Root #2776 / Delivery #2777：托管插件后端接口可独立注册，不强制后台设置页。权限管理增加「插件」页签，按稳定插件标识分组、显示插件名。用户特别确认：若同时声明后台设置，两处编辑必须等效，共用同一权限策略，不能形成两套授权；设置声明只增加可进入编辑的位置。授权迁移保留 enabled/full/custom/operation 明细，日历移除虚假设置入口。Root负责隔离assembly与集中QA；无承诺完成日期。当前正在验收，source integration、push、正式插件包发布与部署分别核验；后续只以Root及当前Git/证据判断完成状态。

### Plugin policy source integration
同一冻结候选 c018ef981 已通过集中Dev Acceptance：本地72前端、176Rust、7日历业务、8SQL场景；CI run 41667062386追加36次宿主用例执行（35唯一），含无页面boot和导航/接口同源权限。源码已本地FF合dev；插件候选6fd8bd2保留remote价格/OpenAI更新后合main为fa6cf913。完整导入业务E2E、部署态HTTP矩阵、浏览器视觉与正式插件发布不在已完成证据中；未重启服务/发布安装包。最终push状态见Root #2776与实时Git，不从本条推断部署。

## 2026-11-04 18 Editable assistant completed
用户确认整个 AI 解读助手应由 MCP 可编辑页面源码承载，并保留实时排盘数据，动机是后续可直接修改助手而不受插件内部硬编码限制。Root 已实现 renderAIHelper(data)、可编辑页面助手及复制同一数据快照；插件 0.1.15 本地标准 family switch 成功，目标 block 01a19524-013a-75d7-aacb-e76cb666d2bc 已通过 MCP 更新。3 契约检查、6 双端 factory 场景、实际 3100 桌面/手机交互/剪贴板/刷新、7800 月历/八字/紫微接口通过。插件源码提交 6fae2b6 已合 main 并 push，最终 main 595e3eb5aec0c628d390f0500a912dcb623551a3 含远端无关 pricing 更新；主宿主 dev 未改产品代码。正式签名发布、完整 CI 与云端部署未验收，无承诺日期；证据 tmp/test-governance/calendar-editable-helper/QA_REPORT.md。页面 render 数据不自动持久化；外部 MCP 查询须用明确输入调用现有后端计算接口，不能把浏览器状态当作服务端记录。

## 2026-11-04 20 Migration actions corrected
用户明确旧站签到没有当前业务支撑、黄历跳转在当前页面重复，要求只保留返回今天。Root 删除按钮及签到内存状态/标记，清理公开 onAlmanac 和页面滚动回调；此前自定义引语保留。插件 0.1.16 本地标准版本切换、6 个双端组件场景、3 契约检查及实际 3100 桌面/手机/刷新均通过。源码 bbd5183 已合插件 main 并 push；证据 tmp/test-governance/calendar-today-only/QA_REPORT.md。无新增依赖，后端算法不变；正式云端发布与完整 CI 未验收，无承诺日期。此决策覆盖早期“完整保留旧站签到”的迁移范围。

## 2026-11-04 21 Complete block template registered
用户要求完整区块在代码模板管理中可复用，并由插件安装/升级自动有则更新、无则新增；明确选择旧手工模板备份后移除，只保留官方模板。此决定新增独立 CalendarComposition code_template，保留六个组件的 component 注册，覆盖早期“仅组件入口”对本次完整区块模板的限制，不重复注册每个组件。Root 在本地3100/7800安装0.1.18并验收：4契约、6双端factory场景（前端字节和源码一致可复用）、正常向前升级与重复启用各恰好一条官方模板，真实模板运行预览通过。旧模板全部4修订已备份并删除，备份及QA_REPORT位于 tmp/test-governance/calendar-code-template/；无承诺日期。源码272ab4e已合插件main，保留远端catalog更新后0288f6e已push且确认为远端后续ac30b3c祖先；宿主dev产品代码未改，临时worktree已回收。额外验证发现同版本重传导致artifact/模板不可见、旧0.1.16回退409 managed_execution_retired_or_closing；已用0.1.18正常升级恢复，以上宿主限制未修复，不能宣称重传/回滚通过。正式签名发布、全CI、云端部署仍未验证。

## 2026-11-04 22 Quote revision
用户再次指定中文引语顺序与标点，Root已通过MCP同步当前页面，并更新官方/便携模板。0.1.19本地升级及重复启用、实际页面两处助手和官方模板预览均通过；前后端JS字节未变。源码8268c7c已合插件main，最终push状态与HEAD见 tmp/test-governance/calendar-quote-update/integration.json；完整证据QA_REPORT.md。无承诺日期，正式发布与云端部署仍单独核验。
