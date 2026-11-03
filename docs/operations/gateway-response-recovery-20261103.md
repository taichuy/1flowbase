# Responses 续接归属与原生请求恢复：调查、修复与验收

日期：2026-11-03（北京时间；原始回执保留各自 UTC 时间）。原始问题来自 Codex 线程 `01a19b66-267e-7c31-aba3-57d50b5a039a`。

## 1. 交付状态与范围

业务补丁已从独立 worktree 快进合入 `/home/taichuy/git/1flowbase` 的 `dev`，业务提交为 `ec166726368c1a0db2742a4e5d08ba4a8047847b`。相对开发基线 `1a84d8922035ce3a9fd23af3b0397ee2180fb932`，修改 13 个文件；完整 API 源码树为 `bd7909bbdd0c2f06f2927b42297e161603977e5b`。

OpenAI 插件已升级至 **0.2.81**，本地完成编译、安装和实例切换，随后发布六个平台的正式包，官方 registry 的 latest_version 已回读为 0.2.81。插件本地业务提交 `8403643250601d7713072e49ebea33247c3ede37`；云端提交 `ba384b996882381510bbc981da8f537c7a75c643`。远端原有的模型价格更新被保留。

完整 API 已在独立 GitHub CI 构建成功，构建分支 head 为 `9bbc5e0481ccd638a9a310650ff38613759378f1`，其完整 API tree 与本地业务提交相同。产物为 803,509,960 bytes，SHA-256 为 `8be445b87dc85841f3eeee677ab66a8711cdcff9ffcb266800e965747a9cf937`。7800 已运行该产物：PID `781909`、start ticks `159529229`，health 为 HTTP 200 / `api-server / ok`。7600 仍为 PID `3595451`、start ticks `152462336`，exe/cwd 与切换前一致。

首次 7800 切换并非无故障完成：临时部署脚本在新进程 execve 暂态读取 `/proc/.../exe` 时遇到 PermissionError，过早触发回滚；环境 handoff 的清理时序又使首次自动回滚未启动，7800 曾短暂中断。随后复用项目既有 `buildServiceEnv` 和 prior public_origin 恢复启动，健康与产物身份核对通过。首失败回执与最终恢复回执分别保留，私有环境 handoff 已删除。

原问题线程仍使用 7600 的旧运行实现。7800 的部署不会自动迁移其 endpoint；本次没有修改或重启 7600，因此原线程成功恢复生成不属于已完成的验证结果。

本次只修复已定位的归属与恢复链路。没有新增第三方依赖、数据库表/字段、长期历史副本或任意请求容量上限；没有修改 7600 观察分支及其运行进程。临时 CI 构建工作流只存在于验证分支，不进入 dev。

## 2. 第一性原理：两个问题必须分开判断

### 2.1 409：谁有权认领这份 tool output

`function_call_output.call_id` 只说明工具调用配对，不能单独证明它属于当前网关工作流的待回调节点。Codex 自己执行的工具和 agent-flow 管理的工具，拥有者不同；Responses 是开放协议，客户端也可能发送合法的自含历史。

在 7600 的旧实现中，显式 `previous_response_id` 找不到对应记录后，helper 仍可能按 call ID 回退认领旧 round，再因 response ID 不一致返回 `native_tool_output_response_mismatch`。真实历史数据库中存在 owner/raw 证据为空的 legacy callback，具备触发误归属的条件。官方 Codex 源码还表明：WebSocket warmup 可以使用 `generate=false` 并产生新的 response cursor，不能把这个 cursor 自动当成网关工具回调的所有权证明。

开发基线已经包含 `9999dbc2f`、`a15baba27` 两次归属修复，本次没有重复实现。当前 host 入口的有效行为是：

- `generate=false` 不认领待工具回调。
- 客户端给出 cursor 时，只做对应作用域内的精确匹配，不以 call ID 回退替代 cursor。
- 只有明确的 agent-flow owner 或经过验证的 legacy proof 才可认领。
- 已完成回调的重复提交必须具有同 flow/source 的持久化一致 attempt 证据。
- 完整 call set、key/creator/application 作用域与歧义拒绝继续保留。

这条错误路径已通过受控反例与 6 项编译后行为测试验证。原会话 JSONL 与现存 API 日志没有保留原始 409 请求/响应的逐字节 wire；数据库中的大型 native body 也只保留 digest/size。因此，这部分结论是“真实历史状态支持的可复现代码路径”，不能称为原 409 报文的逐字节重放。

### 2.2 断流：收到临时错误后，还有没有合法的恢复动作

本次历史调查与受控对照定位到两种独立缺陷：

1. 插件把非 terminal 的 HTTP 502 归为终止类错误。记录中 `semantic_terminal=false`、`failed_after_first_token=false`，仍提前终止，未用完已有预算。
2. 即使插件正确给出 `logical_invocation_retry`，宿主对 `NativeOpaque` 一律使用不可重建判断，拒绝了某些可以按原样重发的请求。

原 WebSocket 1013 → HTTP 路径已消费物理尝试，Host 和 Provider 的计数必须保持同一个预算；不能由插件另开隐藏循环，也不能在回到宿主后补充已消费计数或重新计算绝对 deadline。合法外层重试由当前 ledger 轮换 transport epoch，不能从旧 snapshot 恢复旧 epoch；每份 receipt 必须匹配它所属 attempt 的当前 directive。

所以需要区分两个能力：**按原样重发 host 持有的请求**，以及**理解并重建完整语义上下文**。这次只为严格受约束的前者授权，native 模式仍然是 opaque，没有获得 configured semantic retry 的能力。

## 3. 实现与三层职责

### 协议映射与 Host 所有权

`ProviderTransportPayload` 继续由 Host seal。只有当前支持的 Responses 状态引用满足以下条件时，才具备候选资格：wire body 的 `previous_response_id` 和 `conversation` 缺失或为 JSON null；input 是字符串或数组；数组顶层不包含 `item_reference`。这些检查不递归解释 opaque 历史。

实际选中的 native transport 必须同时匹配 protocol、digest、size 和完整 wire body；不能仅相信可变公开结构中声称的摘要。Canonical previous response 也必须为空。默认 trait 实现返回 false，其他 Provider 不会被动获得新权限。

独立复核发现，matching `client_protocol_envelope` 的 body 也能参与最终请求合成。因此同样检查其 `previous_response_id`、`conversation`。Foreign envelope 按插件已有忽略语义处理；合法未知字段、header/query 中同名的 conversation 扩展不被误禁。插件原有 typed-field collision 校验没有放宽。

### AI Native 恢复决策

授权仍需通过现有 commit/terminal、epoch、attempt budget、absolute deadline 和 token bucket 等 gate。提交后、语义终止、预算耗尽、过期或 epoch 不匹配时仍停止。没有增加默认尝试数，也没有延长 deadline。

自动恢复使用首次选中且完成 pipeline/协议上下文解析后的完整 `SelectedLlmInvocation`，连同 route/runtime、debug context 及 native selection 前的 transcript/tools 投影。恢复时不再执行可变 pipeline，不重新读取 protocol locator，不重新选路，也不重新进行 engine selected preparation。

每次只刷新当前 `provider_attempt_index`、`provider_invocation_id` 与 recovery directive；原先缺失的 deadline 仍由同一个 ledger 的冻结绝对时间补入。不会从旧 snapshot 恢复旧 epoch、清除 commit/terminal、补充 attempts 或延长 deadline。

独立复核还发现，单独 pin route/config 不足以保证请求不漂移：合法的 state/time pipeline 可以改变 body/query/header 或 business context。这也是本次保存完整 selected invocation 的原因。Configured semantic retry 继续 fresh build、执行 pipeline 和重新选路。

选定 route 内使用 scoped `Arc<tokio::sync::OnceCell<Value>>` 固定首次成功维护后的 effective config/account/credential。原有 OAuth、required fields、managed-secret CAS 错误仍可见；失败初始化不缓存，新 configured route 使用新 cell。生命周期仅限这次 logical route，没有新增 DB 历史或长期缓存。

### Provider 插件

0.2.81 仅修正错误分类：typed unavailable，或 typed UpstreamError 且 HTTP status 为整数 500–599、`semantic_terminal` 明确为 boolean false 时，保留可恢复语义。显式 terminal 与已提交错误继续优先停止；missing/null/string/number 的“false”不能被误当作 false。

插件不增加内部 retry loop，不自行分配额外预算；保留 first failure、raw body、原错误 message 和物理尝试回执。通信分发与宿主预算管理继续由基座负责。

## 4. 从规范到机器检查

可以把准入条件写为下列合取，任一项失败就不能得到自动重放授权：

`HostOwned ∧ NoResponsesStateReference ∧ SameSelectedInvocationExceptAttemptMetadata ∧ SameRouteConfig ∧ ReceiptMatchesCurrentEpoch ∧ BeforeCommit ∧ NotTerminal ∧ HasBudget ∧ BeforeDeadline ∧ TokenBucketAllows`

这里的 epoch 一致指 receipt 与本次 directive 一致，不要求两次外层尝试使用同一个 epoch。这表达当前协议与实现边界，不保证模型输出确定性，也不把未建模的未来协议扩展说成已证明。

业务修改之前由独立 agent 建立可执行有限规格，再用 Rust 测试把关键条件绑定到真实入口、真实 resolve/invoke 路径和状态变化。有限模型不是对整个 Rust 程序的形式化证明；两类证据分别保留：

| 检查域 | 机器检查结果 | 边界 |
|---|---:|---|
| callback admission | 294,000 个 current-host/spec 组合零分叉；4/4 故意突变检出 | 以 DB summary/raw 一致等显式前提为范围 |
| HTTP 恢复分类 | 651,264 个分类组合；Host 3,072 个组合；8/8 突变检出 | 保留 terminal/post-commit 与不同 JSON 类型反例 |
| native exact replay 初版 | 195,648 个分域检查零分叉；33 个 fixture；10/10 突变检出 | owner、binding、ledger、gates 分域，不冒充全仓覆盖率 |
| envelope 授权闭包 | 768 个组合：旧模型 16 个可构造 wire 的错误授权 → 0 | matching envelope 合成语义被纳入授权 |
| selected snapshot | 2,048 个状态：旧模型 880 个 effective request 漂移、1,022 个完整 snapshot 漂移 → 0 | 包含合法与对抗状态，不全部等同于生产可达路径 |
| configured 与元数据保持 | 1,024/1,024 configured fresh 行为保留；144 个 metadata 检查零分叉 | 不还原旧 epoch、不补预算、不清 commit、不延 deadline |
| snapshot 扩展突变 | 14/14 检出，原 10/10 仍保留 | 包括能力回退、错误拒绝、快照失真，非全仓 mutation coverage |

验证脚本、JSON 报告、运行输出与 SHA manifest 保存在证据目录的 `verification/` 中，能够独立重跑。真实 Rust fixture 包含 stateful pipeline：两次调用之间故意改变合法上下文，比较实际传入 ProviderInvoker 的完整 input，剔除明确的三个 per-attempt 字段后要求相等，同时验证 configured retry 仍取得新内容。

## 5. 编译后测试与独立复核

各 suite 独立结算，避免重复 filter 被错误累加：

| 范围 | 结果 | 命令总耗时 | 最大子进程 RSS |
|---|---:|---:|---:|
| control-plane-contracts 全 lib | 68 通过 | 4.975 s | 767,480 KiB |
| orchestration-runtime 全 lib | 629 通过，1 ignored | 60.378 s | 2,055,380 KiB |
| control-plane provider_invoker | 105 通过 | 25.768 s | 3,214,156 KiB |
| client_ownership | 6 通过 | 0.307 s | 97,608 KiB |
| OpenAI 插件全 lib | 247 通过 | 16.206 s | 800,612 KiB |

ignored 是 `profile_workflow_graph_pressure`，原本就只用于显式 profiling。插件原 244 项全部保留，新增 3 项；旧插件业务代码对前两个新回归测试为 2/2 预期失败，修复后通过。真实 WS 1013 → HTTP 502 测试核对同一预算、没有插件内部额外连接以及 first-failure 保留。

另有一个本次写集之外的既有失败：`native_reused_node_persists_distinct_callback_wait_occurrences` 的 prompt token 为 22、预期 11。为排除 shared target 的错误缓存归因，明确清理三个相关 crate 后，从 clean dev 基线强制重编译，仍复现相同失败；总耗时 51.565 s，最大子进程 RSS 4,104,808 KiB。早先复用候选 binary 的一次“基线”结果被排除。这一旧失败没有被改断言掩盖，也没有扩展本任务去改 billing。

所有业务修改冻结后，由同一位 fresh QA 复核。首轮提出 envelope 授权与 pipeline 漂移两个 blocker，修复后由同一 QA 重审；最终 V3 仅补齐两个测试 fixture 的真实 node/artifact 前置，共四行，业务 SHA 未再变化。没有以删测试、弱化断言或改生产入口规避 fixture 失败。

最终 QA 直接核对了 13 个源码文件的 SHA、CI source head/API tree、产物 SHA/大小、最终进程身份，以及 native 前后请求 SHA 和完整 provider_binding/configuration_digest。结论为：业务修复、受限恢复行为、完整 API 构建及 7800 部署通过；没有未关闭的产品代码 blocker。上游失败探测、末次成功生成及原始会话的生成结果按各自观察单独结算，不能由这些通过项互相推导。

## 6. 真实请求、部署与上游对照

使用 `gpt-6-luna / max`。native 对照请求包含合法的 `truncation: "disabled"`，从真实映射入口选择 native transport，且不带 response/conversation cursor 或 item reference。修复前后 HTTP 请求体摘要固定为 `f89152b0d1a3615746c96750089f22d631b4e532891a335ed3e892f30a2fc85f`。

修复前已运行的 7800 使用插件 0.2.81：flow `01a19f44-e60a-7734-aafd-1cf3b0ca7baa` 收到非 terminal HTTP 502，Provider 已正确返回 logical invocation retry；Host 仍以 `non_reproducible` 停止。Native policy、lifecycle-only commit、总预算 4、实际 charge 1 都已从 PG 回读，耗时 11.923 s。这是宿主缺陷与插件分类缺陷分离后的真实反例。

修复后的相同请求为 flow `01a19f57-74af-74fb-a2e2-7d928625a91f`。PG 回读确认 provider instance、插件 0.2.81、installation、artifact checksum 和 effective configuration digest 与 before 完全相同。

| 同一 native 请求 | 旧 Host | 修复后 Host |
|---|---:|---:|
| Host 最终决策 | non_reproducible | attempt_budget_exhausted |
| 最终 outer attempt | 0 | 1 |
| 物理 attempts 消费 | 1/4 | 2/4 |
| 用时 | 11.923 s | 22.067 s |
| Provider 错误 | HTTP 502 | HTTP 502 |
| SSE 语义输出 | 无，只有 created/failed | 无，只有 created/failed |

这里的 **2/4 是正确的既有边界**：源码 `AUTOMATIC_TRANSPORT_RETRY_LIMIT = 1`，首次调用后只允许一次外层自动重试。两个 HTTP logical invocation 各消耗一个物理 attempt，第二次后 outer quota 已耗尽。`AttemptBudgetExhausted` 同时表示外层额度或物理总额度耗尽；总上限 4 不是要求必须发满 4 次。这次观察证明错误的不可重放阻断已消除，并实际执行了允许的一次重试；没有增加默认额度，也没有生成成功。

交付前观察到本地 Codex 已能返回模型文本后，再运行一次相同 `gpt-6-luna / max` native 请求。请求 body SHA 仍与前后对照完全相同；7800 返回 **HTTP 200**，完整收到 **response.completed**，最终文本严格等于 **GATEWAY_PROBE_OK**，耗时 **5.116 s**，共 35 tokens（input 14、output 21，其中 reasoning 9）。response ID 为 `resp_01a1a018-1cda-716c-b45a-ec89744eaaaa`。

这是修复后 7800 在交付前的真实成功生成证据。先前的 502 前后对照继续用于证明恢复决策变化；这次成功请求不能证明“曾经失败的同一次请求重试后成功”，也不能解释为原 7600 历史线程已经迁移或恢复。三个阶段分别保留，没有覆盖失败回执。

此前同一 Provider instance/account 的直连对照分别返回 HTTP 502：普通请求 10.449 s、Codex 形状请求 10.484 s、显式走本机 7897 代理 11.465 s，错误均为 upstream temporarily unavailable。由此不能维持“本次测量时直连健康”的假设；同时，这个事实不改变已验证的网关提前停止缺陷。

已有 semantic smoke 显示，插件分类修复使错误终止的 charge 2/4 变为预算正常耗尽的 4/4；连续失败耗时从 24.672 s 增至 47.619 s。它证明恢复动作被执行，不证明响应加速，也不能代替 native 验证。

本地 Codex 协作使用持久化线程 `01a19eeb-100d-7bf3-8851-f59fe73b22c3`。早期调用受到上游 502 阻断；切到 7800 的一次调用在 277 ms 返回 `unknown_model`，错误原文为 `model is not declared by the active publication`，因为当前 7800 发布未声明 `gpt-6.1-sol`。错误 turn、空 messages 和模型路由回读均单独保留。

最后沿 Codex 原有 7600 模型入口，正式 resume 回读确认 **gpt-6.1-sol / high / danger-full-access**。匹配 client message 的 turn `01a1a015-091a-7dc1-a794-864386215a53` 在 25.077 s 完成并输出中文分析：物理总额度是上限，不是必须用满的目标；剩余物理额度不能扩大 outer retry 权限；sealed body 含 previous_response_id 即使 canonical 字段为空，也应拒绝 AutomaticTransport。

这次协作是对已提供源码规则与回执的独立逻辑核对，没有调用工具、修改文件或重启服务，也不冒充本地 Codex 亲自完成了实施或部署验收。该成功的 gpt-6.1-sol 调用与先前 gpt-6-luna 的失败发生于不同时间、模型及端口，不能相互替代。

## 7. 资源与量化结算

本机所有重型 Cargo 串行执行，单次独立 cgroup，CPUQuota 200%（宿主 20 CPU），禁止 swap。最终 API 在 6GiB/7GiB 限制内的四次尝试均在 rustc codegen 触发 OOM；没有把这些失败记为通过，也没有继续提高本机资源预算。旧 7600/7800 在这些构建期间保持原进程身份。

后续核对到，当前旧 7800 产物此前成功构建的 cgroup peak 为 **9,999,908,864 bytes**，原上限约 12.3GB。因此之前的成功产物不能证明完整 API 可在这次本机 8GB 约束内重编。候选改由独立 GitHub runner 构建；这不属于“本机 8GB 构建通过”。

最终独立 CI 使用 Rust 1.99.0、Ubuntu 24.04、jobs=2、no incremental、API codegen-units=256/debug=0，保留完整业务 features（包括 `tikv-jemallocator/stats`），未通过删功能规避构建。Runner MemTotal 为 16,367,524 KiB；编译耗时 475.14 s，最大子进程 RSS 7,919,708 KiB。Run wall time 531 s、构建 job 实际持续 529 s。

最终交付的三个 workflow 合计 **2,423 s = 40.383 raw runner 分钟**：插件 CI 14 s、正式发布 1,880 s、Host 构建 529 s。三个 run 分别只有一次最终执行，不把本地失败、push CI 或发布等待混作 PR 反馈样本。

| 用户要求的指标 | 修改前 / 对照 | 修改后 / 本次结算 |
|---|---|---|
| 已知缺陷检测保留率 | 未修改原门禁；未建立全门禁独立缺陷样本与检测项映射 | 原 244/244 是回归测试保留并通过，不等同缺陷召回率。本次披露的故意突变集合分别 4/4、8/8、10/10、14/14 检出，各集合内为 100%；不外推全仓检测能力 |
| PR 反馈 P50 / P95 | 本次未建 PR，样本数 0 | 样本数仍 0，分位数无定义；不能用 push CI 替代 PR 等待时间 |
| push CI 实测 | 与 PR 样本分别结算 | 本次 provider-ci wall time 11 s；只有 release-contract/detect，package-dry-run 与 actual-package-conformance 为 skipped |
| Runner 总分钟数 | 未改变门禁，不能假称减少 | 插件 CI 14 s + 正式发布 1,880 s + Host 构建 529 s，共 2,423 s / 40.383 raw runner min |
| 本地峰值 / OOM | 旧 API 成功产物峰值约 10.0GB，超本次限制 | 本次受限组最高 7,516,192,768 bytes；OOM 4 次；专项测试成功、本地 API 构建失败与独立 CI 构建成功分别计数 |
| 治理代码净变化 | 现有门禁实现与配置 | 进入 dev 的治理代码净变化 0；临时 CI 不合 dev；未宣称删除旧门禁 |
| 业务与测试代码 | 本次两仓基线 | Host 业务 +239/−100，插件业务 +17/−6，合计业务净 +150 行；测试净 +1,065 行；版本行 +3/−3 |
| 数据库存储结构 | 现有 DB/ephemeral 方案 | 0 个新增表/字段，0 份新增长期上下文副本 |

这里 runner 分钟为各 job 实际持续时间之和，不含平台账单倍率或舍入。插件发布 wall time 为 469 s，六个平台及 registry 均成功。本次没有改变门禁算法，因此不存在可报告的门禁加速或计算资源节约结论。

完整 invocation 的 `Value` clone 是 **O(I)** 时间与空间，I 为本次完整调用大小；不能把固定数量的引用说成 O(1) 字节。wire body 等值比较为 O(B)，顶层 input 扫描为 O(n)。自动重试省去重复 pipeline、locator 与选路 I/O，但新增快照持有成本；没有测量成功负载下的吞吐/RSS/尾延迟，就不声称这些性能指标下降。

## 8. 按四个标准复盘

**顶尖实践多做什么：** 先建立 ownership、commit 和 replay 身份的不变量；把“有重试意愿”与“有合法重试权限”分开；主动寻找错误授权与请求漂移的反例；保留失败的证据；用旧基线重编排除测试缓存污染；确认部署产物来源，而非只看代码 diff 或 exit 0。

**从什么一手材料开始：** 指定线程的原生 JSONL；两个独立 PG 数据库的实际记录；对应运行分支与进程指纹；官方本地 Codex 的 warmup、retry、SSE fallback 与队列/恢复 RPC 源码；Provider error factory、Host ledger 与实际请求合成代码；内核 OOM 日志；GitHub 原始 run/job/release/registry 回执。

**具体方法与工具：** 最小受控反例、有限域穷举、故意突变、红绿回归、真实 resolve/invoke 路径 fixture、单变量构建诊断、cgroup/kernel RSS 取证、完整 API tree 和文件 SHA 绑定、同请求体 native 前后对照、独立 fresh QA。全程仅 Root 装配与远端执行，避免多 agent 竞争构建或写同一 worktree。

**除你我外的证明：** 可重跑的 verifier；编译后的 Rust 测试日志和非零测试数；有效 clean baseline 失败对照；相同 API tree 的独立 CI；带 digest 的正式插件发布；PG 中实际 charge/decision/commit 回执；部署前后的进程身份与健康检查。每项结论都对应其证明范围，机器检查与运行观察相互补充，不把有限模型包装成整个程序的完备证明。

## 9. 证据索引

宿主证据根：`tmp/test-governance/gateway-response-recovery-20261103/`（保留在开发 worktree）。原始会话/DB 内容仅保留在本机私有证据文件，不进入提交或 CI。

- 候选：`candidate-v3-manifest.json`；diff SHA `c82b5e1d9fe17bcc039c58d0f9f9f5d2631f5dfb78404666ffc24f1631cabfc4`。
- 规格：`verification/*_verifier.py`、`*_report.json`、`*_run.txt`、`verification/manifest.json`。
- 行为：`v2-contracts.{json,log}`、`v2-runtime.{json,log}`、`v3-provider.{json,log}`、`admission-behavior.{json,log}`、`plugin-suite.{json,log}`。
- 基线：`baseline-callback-forced.{json,log}`；原先无效 cache 结果不作为证据。
- 请求：`native-before-smoke.{json,sse}`、`native-before-flow.json`、对应 after 文件，以及 `native-before-ledger.json`、`native-after-ledger.json`；三个 `direct-*` 对照文件；交付前额外健康探测 `native-final-smoke.{json,sse}`。
- 构建来源：`api-build-selected.json`、`ci-provenance.json`、`ci-download-receipt.json`、`ci-evidence/`。
- 部署：首失败 `deployment-7800.json` 与最终恢复 `deployment-7800-final.json`。
- 汇总：`test-result-summaries.json`、`local-resource-failures.json`、`plugin-release-final.json`、`plugin-registry-final.json`。
- Codex：`codex-final-resume.json`、`codex-7800-failure-read.json`，以及 `codex-existing-route-{dispatch,resume,result}.json`。
- [插件 CI](https://github.com/taichuy/1flowbase-official-plugins/actions/runs/41563398572)。
- [插件正式发布流程](https://github.com/taichuy/1flowbase-official-plugins/actions/runs/41563398384)。
- [OpenAI 0.2.81 Release](https://github.com/taichuy/1flowbase-official-plugins/releases/tag/openai-v0.2.81)。
- [冻结 API 候选构建](https://github.com/taichuy/1flowbase/actions/runs/41567141802)。
- Cargo 参数含义以 [Cargo Profiles 官方文档](https://doc.rust-lang.org/cargo/reference/profiles.html#codegen-units)及本机 Rust 1.99.0 参数输出为依据；四次本地失败仍按失败记录。
