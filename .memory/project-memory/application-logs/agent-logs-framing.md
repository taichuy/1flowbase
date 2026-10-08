---
title: "Issue 2282 Agent Logs 应用需求对齐"
memory_type: project
created_at: "2026-10-07 16"
updated_at: "2026-10-08 20"
decision_policy: verify_before_decision
status: verified_platform_distribution_and_stateful_collection
tags: [issue-2282, agent-logs, client-trajectory, ingestion, collector-cli]
---

## 用户目标与边界

用户于 2026-10-07 明确提出扩展 #2282：新增 `agent-logs` 应用，参考 agent-flow 应用壳和三级日志（记录摘要 → 对话 → 客户端轨迹），纯采集、不执行工作流；侧栏是日志、API、采集 CLI、统计。

底座维护协议数据、权限、持久化和读投影；已有脚本能转协议时复用，否则通过格式适配插件接入不同 agent。共享通信、子进程管理及 SDK 能力由底座承担，插件保持单一。

用户随后明确纠正：供应商模型价格缺少时直接复用已有默认计费模型 `zero / any`，费用为 0。该要求替代前一轮 Root 建议的“费用为空、绕开全局零价”；不得再次以 Langfuse 缺价格语义覆盖用户要求。复用定价计算不代表授权对导入日志扣余额或生成实际账务。

第二层对话详情按 agent-flow 的轮次阅读方式：系统提示词 + 该轮用户输入 + 该轮最终 AI 回复；多轮会话每轮只投影一组问答。中间 AI 回复、工具过程、子代理等完整事实留第三层客户端轨迹。这里的“压缩”是展示投影，不新增 AI 总结请求，不删除底层事实；缺最终回复不拿中间回复冒充。两项修正是已确认硬边界，不再处于待确认状态。

用户暂停后连续提供现有Agent Flow列表/对话详情和总轨迹截图，明确三层均已有、应直接复用UI交互与存储。Root收敛复用边界：同一ApplicationLogsWorkspace/Table/DetailPanel/FloatingWindow与ClientTrajectoryWorkspace/Detail，同一application_run_log_tasks、conversation_message_items、client_trajectory目录/正文结构；仅演进采集事实接入、记录归属与run专属读取依赖，禁止采集专属三级UI或复制投影表族。纯采集没有工作流内部事件，展示能力依据真实事实；本地源记录不能补造为HTTP/WS原始帧。用户随后明确“更新issue开工”，已恢复按此边界实现。

## 最新平台分发纠正

用户于 2026-10-08 明确纠正采集器产品流程：远程插件先安装进 1flowbase，客户端安装时从自己的 1flowbase 下载脚本/二进制/校验文件；“全部”展示远程目录及平台真实安装状态。此前静态 Codex catalog 和 GitHub 直下载方案不满足此要求，不能作为最终验收结果。用户已确认复用既有目录/安装/版本/校验/落盘，补显式客户端分发类型、完整多平台包与本地typed下载接口；Root已完成隔离装配、集中源码QA、原分支集成/push、正式发行与实际页面验证；主目录保持原分支。截止日期未设定，动机是让分发由用户自己的平台控制；安装后普通 CLI 安装不再依赖远程仓库在线。原生 Rust 客户端执行与共享 SDK、已有协议及三级日志边界保持。

## 前序原生采集器阶段（历史）

2026-10-08 01：原生源码及本地实际页面已验证并ff合入主dev704618030、官方main28d2450，tracked clean，开发环境官方重启成功。集中源码QA SDK11/adapter9/installer5/API-client3/frontend8、i18n和style4scene及真实双端通过；同源安装端点已修为绝对URL。GitHub4次官方push和1次dev push（含代理）均被服务端500拒绝，Issue更新也失败；实际remote仍99e094459/c9c5bda，未push、未触发release、未生产部署。一键公网安装尚不可用，COL007及非Linux后台生命周期仍未验证；不可据页面URL声称已有公开发行。任务worktree/proof DB/session/service/port已回收，提交和完整证据保留。下次在写入恢复后直接从原两仓push，运行collector-release和实际下载验签，不重复已有效native/页面源码QA。唯一待同步Root草稿和交付状态：tmp/test-governance/agent-logs-native-collector/{root-updated.md,delivery-status.md}；最新源码验收qa3/QA-report.md、主验证final-main/receipt.json。

用户已确认“独立 Rust 采集插件 + 共享 Collector SDK + 统一 Shell / PowerShell 安装器”，要求参考采集器目录和安装详情两张原型开工。首版 Codex，源码位于官方插件仓库 `runtime-extensions/@taichuy/codex-logs-collector`，共享 SDK 位于官方 `sdk/agent-logs-collector`；复用主仓 pinned canonical Rust DTO，不复制协议，不注册服务器 slot。安装器在用户电脑执行，Key 本地输入，按应用 ID 分开配置/服务，升级保留 checkpoint。

Root #2300 继续作为唯一计划与用户验收入口，新增 Delivery #2304（原生采集与安装发布）、#2305（真实后端目录与原型页面）。本轮由 Root 装配并集中验收，主仓保持 dev、官方仓保持 main；截止日期未设定，结束边界为源码验证、主分支集成以及实际官方下载发行证据完成。原因是原 Node 源码命令要求检出仓库，不能兑现客户端一键安装、持续采集的产品入口。

主工作树起点 dev99e094459，官方 mainc9c5bda。隔离 assembly 位于 `git_worktree/agent-logs-collector-main` 与 `agent-logs-collector-plugins`；产品和 fixture 已装配，集中 QA 进行中。本轮尚未集成、push 或发布，不把源码完成当作安装/运行验收通过。具体状态及证据依 #2300 Control Ledger。

## 前序完成阶段与恢复边界

用户最新“合并回来dev分支，启动报错了修复一下”沿用开发库SQL、migration、重启与验证授权，不重复索要恢复批准。Root #2300 是唯一计划与 Control Ledger，Delivery #2301/#2302；三层直接复用、默认零价、最终回复、共享 CLI/Codex adapter 已实现。

主目录 `/home/taichuy/git/1flowbase` 保持起始分支 dev，最终 `00b3daba8784a73ee674cffdfff14644f86a976e` 与远端一致，tracked clean，原私有 untracked memory 保留；已合入并push，未deploy。临时assembly工作树已回收，任务分支保留。实际官方启动、前端3100 Ready、后端7800 health、认证catalog三类型与临时cookie回收均通过。源码功能 scoped QA、测试整库隔离7项、目录候选8项及主恢复有限QA形成有效证据链；Root进入用户验收，Delivery #2301/#2302 已结算关闭。

缺失model_fields/model_change_logs已定向恢复：可信2026-10-03主库clone的324原字段IDs/非空用户配置、39原历史及约束保留；当前22模型展示/40授权保留，project_tasks11字段语义按源码与成功请求重建并明确记录recovery事件。正常migration完成，不reset业务库、不补造旧历史。11字段原新增ID与部分后续元数据历史未完整找回；没有立即丢失前全量快照或完整业务rowchecksum，不能声称历史逐字节恢复。

旧schema-only测试helper的search_path可导致历史migration越界DROP父库public，机制已在独立环境取证，与实际缺表状态吻合；具体触发测试/时间没有完整证据。helper已复用独占整库、显式search_path与稳定维护库cleanup，生产migration未修改。主恢复QA唯一目录漏项已由M2补AgentLogs并关闭。重型CI/并发/release与移动端最终raw像素验证仍有限制。

下一事件仅为用户验收Root；最新Issue Ledger与实际仓库/运行态优先于这条阶段记忆。证据保存在主目录 `tmp/test-governance/agent-logs-2300/`。动机是将未经过网关的客户端日志纳入统一日志资产；用户没有指定截止日期。

## 当前结算与后续入口（2026-10-08）

Root已完成用户批准的平台分发修正。主仓仍是dev、官方仓仍是main，已集成并push；正式六平台Rust发行及目录发布成功，开发环境已按官方命令重启。实际采集页完成正式包安装到当前1flowbase，双语双端交互、全部本地asset的正式字节/哈希、Linux原生--no-start安装及可信签名校验均已取证。平台保留分发包供用户使用，客户端不从GitHub下载；未创建用户Key、未启动用户机器daemon、未生产部署。仅两棵任务assembly已回收，共享node_modules目标保留。

动机仍是平台控制分发与离线可用，三级日志复用、最终轮次回答、默认zero计费、原生SDK/适配器边界保持。Root #2300为唯一用户验收入口，Delivery #2304/#2305的实现/集成/发行已结算；截止日期未设定。具体当前refs、源码/发行身份、QA和未执行分支以Issue最新Control Ledger及tmp/test-governance/agent-logs-platform-distribution/final-main/QA-report.md为准。实际Windows/macOS执行、PowerShell脚本执行、全部权限/跨节点/旧server-runtime/UI错误分支未新跑，不把源码和有限运行证据标成全量DIST通过；后续只依据新问题补受影响证据，不重复有效验证。

## 已批准的增量采集方向（2026-10-08）

用户确认采用 Vector / Fluent Bit 式的有状态文件追踪平衡方案，由共享 Rust SDK 管理来源索引、ACK 游标与最小 adapter context 原子断点、pending 来源区间重放、实际 envelope 字节组批和跨文件按批轮询；Codex 插件只持久化最小格式上下文与转换事实。保持串行持久 ACK、稳定事件身份、原始事实、完整历史前缀检查、已有协议与平台分发路径；不引入 SQLite、并行上传、永久全文缓存或任意总记录上限。

动机是减少重复历史 JSON 解析和上下文重建，同时把功能完整性与恢复可靠性放在性能前面。严格前缀校验仍读 O(H)，不能宣称总 I/O 已变为 O(Δ)；组批是可配置的传输目标，超目标单事件完整发送。断点升级须保留身份。无截止日期，最新验收与发行状态以官方插件仓库 Single Issue #8 为准；已装旧 CLI 需要先升级 1flowbase 中的包，再用同一 installation ID 重执行客户端安装命令。该优化已完成源码验收和正式签名发行，不代表替用户安装或启动客户端。
