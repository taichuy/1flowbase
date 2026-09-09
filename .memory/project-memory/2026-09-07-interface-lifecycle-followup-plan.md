---
title: "接口生命周期后续与插件组合架构计划"
memory_type: project
created_at: "2026-09-07 12"
updated_at: "2026-09-09 07"
decision_policy: verify_before_decision
status: active
tags: [interface-lifecycle, planning, github-issues]
---

## 当前阶段（2026-09-09 07）

用户交付 F01 修复后要求本会话再次独立审计。已核实当前候选、远端beta和最终CI身份，独立复算原始artifact为268唯一Rust（含50必需）及35Node通过；安装owner阻止同版异内容覆盖，历史SQL不依赖当前manifest，正式重装/重启/恢复/并发及旧数据反例有真实证据。本轮结论PASS_WITH_LIMITS，解除此前F01/AC009的red，建议按Root既定有限范围最终验收；Root仍open，用户尚未最终关闭。本轮只读审查与证据整理，未修改产品或线上Issue、未重跑本地重型测试。限制仍含原archive字节严格相同、缺可信旧checksum拒绝、安装发布取消缺专项运行fixture、跨DB/FS崩溃恢复与性能不在范围。原因是核对上轮缺陷是否闭合而非重开整体架构；无固定截止日期。最新证据指针：tmp/test-governance/2007/independent-f01-review/review.md；后续先核对当前Root/代码，不再沿用上一份F01红灯结论。

## 上一轮独立复核（2026-09-08 21）

用户报告 Root #2007 已完成开发/集中 QA 并合入 beta，要求本会话独立检查。本轮已只读核实远端 beta、CI candidate、原始 artifact 的一致性与实际 265 唯一 Rust（含47必需）/35 Node 全通过；四 Delivery closed、Root仍open。复核发现同版本异内容重装覆盖当前贡献 metadata，使旧积压脱离历史查询/清理保护的范围内缺陷，暂不建议最终验收，等待原开发会话有界修复与新反例补验。静态可达链已交叉核实，新增场景未跑PG复现。本轮没有修产品或改线上Issue；无固定截止日期。动机是保住已批准的积压可查、旧目标和退休边界，避免专项CI green被泛化。详细结论与证据唯一指针：tmp/test-governance/2007/independent-review/review.md；后续先核对该报告、在线Root和当前源码，不直接沿用旧“全部AC green”。

## 上一阶段计划（2026-09-08 10）

阶段更新：用户在本会话确认成熟授权模型的平衡方案，并明确要求更新 Issue 计划、再向原开发会话发继续指令。Root #2007 与四个 Delivery #2008–2011 已同步，原生父子关系和正文回读验证完成。当前 REPLAN_APPROVED/phase:ready，P01=a8511d6a 保留在原隔离 assembly，原 P02 退出活动调度，由 P02A→P02B→P02C 接替；全树13个活动Packet、原10项AC及AUTH-01–10有限反例，均未新验收。P02B贡献授权/撤销/修订持久结构、必要追加migration与显式授权operation已获确认，不能再按旧条款7重复索要同一范围批准；具体语义、写集合与新鲜度/撤权边界以在线Root为准。本轮只修改在线计划和私有记忆，没有接管开发、修改产品代码、执行migration或新测试。用户将在原开发会话继续，无固定截止日期。

用户要求整理插件架构现状并将 Issue Tree 发布线上，之后在独立会话开发。当前插件规划唯一真值为 https://github.com/taichuy/1flowbase/issues/2007 ，原生子 Issue #2008（多贡献安装激活）、#2009（真实接口 Hook）、#2010（命名空间可靠事件）、#2011（升级停用与资源回收）。Root 包含现状永久链接、10项AC、有界Packet inventory、集中Test Batch及新会话启动指令；phase:ready，本轮仅发布计划，没有开发/验收新插件能力。新会话用户启动Root后，以在线正文为准固定具体Packet再执行，不逐个请求Delivery批准；无固定截止日期。

动机：基于已调整的接口生命周期开放实际可安装插件组合，避免把已有Graph/Kernel/Outbox代码等同于真实受管插件已接通。Scope与首批契约见Root，尤其真实Create入口的只读Before、同workspace事件、旧版本不可用显式暂停、普通publish与原业务事务分离。上一阶段#1998为独立验收记录，本轮未关闭或改写其状态。Wiki预览：https://github.com/taichuy/1flowbase/wiki/Plugin-Composition-Architecture-CN 。本地研究提交尚未推送不影响在线交接，不应为启动计划顺手推送其他本地提交。

以下为此前阶段记录，判断当前授权与执行状态优先读取在线Root。

用户授权Root继续接口生命周期实现，P6/P7先完成，集中验收后推送GitHub Actions，验证迁移前后输入/输出/状态/error/权限等价。活动工作区固定 /home/taichuy/git/1flowbase_latest，beta；用户明确要求提前合回beta并直接工作，覆盖旧隔离/pre-QA集成规则。无需重复申请实现/有限修复/push授权。无固定截止日期；以实际验收证据完成为目标。

唯一当前执行真值：https://github.com/taichuy/1flowbase/issues/1998 ，Delivery #1999/#2000/#2001。规范：https://github.com/taichuy/1flowbase/wiki/Request-Architecture-and-Invocation-Lifecycle-CN 。当前本地manifest与QA handoff位于tmp/test-governance/1998/test-batch.json及最新qa5-beta-handoff.md，恢复时先核对这些和git/线上状态，不重启旧Packet。

阶段顺序：接口生命周期管理 → HostExtension/RuntimeExtension/CapabilityPlugin三级插件可时空组合性开放。P6a已撤下且从未实施；空扩展plan合法，适用绑定计划不能绕过；受控Kernel证据不能冒充生产插件加载。完整Outbox/PluginData及插件装配另行审计，延期不等于通过。

P1–P7及原有限修复已在beta集成。用户随后授权修正后端Seed并补齐在线协议，前端不处理；Seed恢复固定官方发布字节，当前候选66911e14aa5cfdb983b3680b96fb41312f50caad。2026-09-07独立在线协议run34131244984成功，唯一fresh QA结算AC009按已批准有限第一阶段场景PASS_WITH_LIMITS。AC001–007明确复用8efaae3的源/fixture一致证据，不能冒充本候选重跑；当前Seed本地14+2/CI解码与在线协议实际通过。完整证据及限制：tmp/test-governance/1998/ac009-current/final-qa.md、execution-receipt.json、baseline-contract-map.md。当前verify34129516604仍因既有schema/前端失败；历史PG/coverage没有本轮重跑。Root与三个Delivery保持open/phase:qa，等待用户只验收Root，无活动开发/测试/自启服务。后续不得把声明16oracle/20error/12count_tokens当作实际线上矩阵；characterize282中225经Gateway、57直连mock WS，真实Gateway WS仅独立成功/错误2例。当前结论不是全接口/全输入空间或性能等价证明。

既有schema4、前端、official Seed integrity、PG旧升级fixture债务已单列；不要擅加scope/migrations、更改翻译或跳过完整性检查。最终结论须区分当前接口回归与全局门禁旧债。旧隔离tree /home/taichuy/git/git_worktree/interface-lifecycle-1998只读保留，结束前按用户规则决定清理；私有memory不提交。


后续批准（2026-09-07）：用户确认继续补齐已列覆盖缺口，含create字段顺序/返回落盘ID/完整授权与错误、Compact原false能力、真实Gateway WS并发/断开/中断/协议错误、完整20行上游错误矩阵，以及schema分类、旧PG升级fixture、CI空Seed过滤器。Root仍#1998，当前Control Ledger已更新；新增执行以Root最新正文为准。主beta保持不变，隔离assembly在/home/taichuy/git/git_worktree/interface-parity-followup（codex/interface-parity-followup），集中一个fresh QA，本地不跑重型Cargo，候选临时分支CI取证后再合回beta。新范围验收尚未完成，不能沿用上轮有限PASS提前结算。无固定截止日期。


2026-09-08续：隔离候选9a2957c6a集中CI34138617197仅WS lifecycle3行失败；Rust新增23/9/4/2与真实error20行24尝试通过，schema本地0 findings。Root最新F1/F2修正保持原业务语义：同provider池本来串行；连接断开!=显式业务cancel，新增确定性terminal前barrier和Nativecancel分开取证；修复已建立typed stream的completion因bridge abort被丢弃（独立owner收尾），不新增自动业务取消。失败明细必须保留。尚未合beta；以#1998当前fix batch为准，严禁把目前候选说成完整通过。


## 2026-09-08 01 本轮结算

Root已完成批准的后端一致性补验、认证schema分类及旧PG/Seed测试前置修正，b1135c206已合回并推送beta。同SHA CI34143783378与fresh QA2通过；报告位于主工作区tmp/test-governance/1998/parity-followup/qa2/final-qa.md。发现的既有WebSocket收尾owner缺口已修复；断连与显式业务取消保持分离。Root/Delivery仍open等待统一用户验收；前端、全仓verify、性能等价、完整Outbox/PluginData仍不在本轮。后续判断先查当前Issue/代码/报告，不把有限范围通过泛化为所有接口完全一致。

## 2026-09-08 08 阶段转入插件架构研究

用户表示对应接口生命周期调整已完成，要求使用 problem-framing 开始研究三级插件时空可组合性架构，重点为接口生命周期各阶段接入、事件注册与通知；随后明确研究范围包含插件自行定义事件并供其他插件订阅。动机是基于已有接口生命周期继续开放真实插件组合能力。本轮仅研究与方向对齐，不代表已批准实现、开放普通插件全局扩展点或关闭既有 GitHub Issue；无固定截止日期。后续不能沿用第一阶段“暂不研究插件开放”的范围，也不能把包含插件间事件理解为已批准具体命名空间、交付或升级方案。

用户进一步要求先重新判断三级分类本身是否必要，指出最初动机包含规避 Rust native so/dll 反复加载卸载时 TLS/static 与依赖全局状态风险；当前主要开发 RuntimeExtension，认可可信 HostExtension 随宿主整体重启的边界，质疑 CapabilityPlugin 的独立价值。此时需区分消费/贡献语义与执行/回收机制，不能仅复述现有分类便宣称三级合理。

随后用户明确认可“HostExtension + 受管插件”两种治理边界：不再把 RuntimeExtension 与 CapabilityPlugin 作为顶层互斥分类，供应商、节点、组件、Hook、事件等作为贡献类型，执行方式与激活作用域分别建模；可信 native HostExtension 继续随宿主重启，现有 Rust 子进程可复用，不因分类调整引入 WASM/Lua。最终类型名称和分类迁移尚未批准实施。用户继续提出是否需要统一技术总线管理插件/事件的注册、时机和通知获取；当前在对齐 Extension Bus 的治理职责与各执行/交付通道边界。

用户认可统一总线建议并要求全面研究可借鉴的成熟架构、算法、数据结构与数学建模。研究入口为 docs/architecture/plugin-composability-research.md（研究建议，非实现或形式化验证通过）：覆盖 OSGi/VS Code 声明与生命周期、typed graph/约束、策略交集与读写集、不可变快照、乘积状态机/TLA+、事件偏序、Outbox/Inbox、fencing、Little 定律与背压，明确各模型假设和适用边界。具体模型选择、协议、迁移与执行计划仍待确认；不把“认可建议”扩展为实现授权。
