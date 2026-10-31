# AI Gateway 透明性与功能回归审计 — packet-t

2026-10-31。接续候选 `b59f505834c5e6f5bf001a83218d242c4081dac9`；保留原 report.md、packet-s、原始记录及已交付的 9 页 PDF。本报告不重做内存收益实验，不将旧 24 次固定完整请求对照解释为部分上下文透明性证据。

## 结论与适用范围

本轮确认 **1 项 A/D 共有的既有兼容性缺陷**：没有 parent、已在本请求内成对出现的工具 call/output，在没有后续 assistant 时会被 `native_tool_output_unknown` 错拒。H/C/S 未引入该缺陷；本轮不修 classifier，不将它混入已量化的内存候选。

在已经接纳的原生路径，新回放没有发现缩短/压缩后的 input 被旧数据库归档补回、工具 item 重复插入，或 H/C/S 破坏本次所验行为。该结论限于下述路径、配置与有限样本，不能覆盖上述真实错拒，也不是全网关功能完整通过。

核心矩阵 A/D 各 40 个场景，共 80 场景/62 次上游调用。原自动断言 78 通过；D 的两个 Native partial 断言误用了内部状态名，原 raw 的公开 `waiting`、required_action 与零上游调用符合规范，另存复核，原失败不改写。最初 D1 14 场景原为 10 通过、4 失败：**2 个合法 seed 错拒是真实产品缺陷，另 2 个 Responses partial 预期错误才是 fixture 问题**。后续仅追加 assistant 的形态通过，不能撤销原始合法输入的缺陷。

主审授权成对补证另计 16 逻辑请求：8 次合法形态被误拒、8 次 assistant-only control 成功（8 captures）；最后正式 cursor 补证另计 8 逻辑请求/8 captures。三组总计 104 逻辑请求/78 captures，仅作为执行量，不写成“104 个产品兼容测试全部通过”。初始 D1、失败的 root-paired-a/a2 试运行仍单列保留。

H/C/S 保留测试二进制本轮重新执行 72 项：Host 53、pipeline 13、真实生产 pool/SQLx 0.9 + 专用 PG 6，均通过。939 相关定向 CI 新执行 475 项通过、0 失败、1 个显式 RSS 压力门禁忽略。最终独立 QA 直接检查实际 I/W 与持久身份；详细范围在下方登记，未执行的测试不计通过。

H/C/S 候选 `b59` 及产品 diff 0 保持冻结；没有改产品、池大小、容量、TTL、schema、插件版本，没有合并、部署或重启生产。classifier 的最小独立修复规范列在下方，**尚未实施**。最终决定仍由用户保留。

## 独立规范与出站检查

以客户端请求 I、实际 mock 接收到的最后出站体 W、逐项声明的映射 M，验证 **W=M(I)**。JSON 对象 key 顺序不作协议要求；数组类型/数量/顺序/内容、字符串及其内部工具参数格式、opaque encrypted_content 均精确比较。没有递归删除 ID、时间戳、unknown 字段，也没有用 A/D 相等替代规范。

本轮普通原生路径仅允许：

| 字段 | 允许映射及依据 |
| --- | --- |
| 根 model | 公共模型 `1flowbase` → fixture 明确配置的上游 `gpt-6-luna`；不进行模型真调用 |
| 根 stream | 流式调用的 `true`；WS typed ingress 与插件 streaming builder 的既有职责 |
| 根 type | WS envelope 使用 `response.create`；HTTP 没有 WS envelope type |
| 根 previous_response_id | 仅映射到该已拥有 parent 的真实 upstream response ID；捕获的响应 ID 与后续请求逐条关联 |

其余根字段及完整 input 不允许变化；例如 store、tools、工具 arguments 字符串、opaque 内容全部保留。Native `/runs/{id}/resume` 是独立协议：response_payload 进入持久 inbox，部分/相同重复提交不调用上游，完整集合才形成一次有序工具结果续接；不将其误称为原生 wire 逐字透传。

DB 独立性限定为：明确引用的 parent/checkpoint/affinity 与当前编排配置一致，仅未引用分支、压缩废弃前缀或另一会话历史不同。A/D 均在同一 disposable DB 内先发送 I，再经真 API 增加另一会话及同会话旧分支，再发送完全相同 I，W 原始 bytes/hash 相同。导出证明 seed/before/branch/after 同 provider_session_identity、other 不同；旧 user/assistant/output 已落库，旧 arguments 的落库没有由这些导出证实。两版各 20 个 flow 的 external/assistant conversation ID 都是 null，**这不是 external-conversation 历史恢复的运行测试**。

负控将未授权旧 user item 加到实际捕获 W 的副本；checker 拒绝。它证明 oracle 能识错，不是生产故障注入。

## 从 Codex 到上游的源码链

1. Codex `codex-rs/core/src/client.rs:1500` 只有在配置一致、当前 input 扩展上一请求及完成 output 时才计算 delta；`:1543` 返回合法 previous_response_id + incremental items。`:2092` 失败时改发当前 request.input，`:2157` 明确二选一。`compact.rs:422` 调用 replace_compacted_history 替换 active view；网关不能因为持有归档就恢复已丢弃前缀。本机 Codex HEAD `6ae8b8aa7c608730d4315d59c5c081897ae02780`，两个已读文件无工作区改动，文件 hash 存在 source-audit.json；这是本机源码证据，不假定所有客户端版本相同。
2. HTTP `api/apps/api-server/src/routes/application_public_api/openai/response_dispatch.rs:81` 解析 I，构建 `OpenAiResponsesEnvelope`；WS `responses_websocket/schema.rs:56` 仅去 envelope type，actor 保留本次 response，不拼接历史；`turn_bridge.rs:64` 进入同一个 prepare_typed_response_turn。session_context 生成 application/key/workspace scoped 身份，头部本身不是 callback 授权。
3. `control-plane/.../compat/openai/responses_index.rs:120` 持有 raw body 与内容无关的位置索引；`request_translation.rs:339` 从该 body 映射 canonical 辅助字段，`:463` 的 previous history 仅来自 previous.answer 的一条 assistant（`response_input/content_mapping.rs:12`），`:603` 保留独立 provider payload。canonical 辅助 history 与 raw wire 是不同对象。
4. `run_service.rs:759` 的 bind_conversation 在明确外部 conversation 且 canonical history 为空时可以查询 DB 填辅助 history；这是一条真实 DB 读取路径，不能被说成“网关从不读历史”。它不重写已捕获的 native wire body。Responses session headers 则主要定义 provider session/transport 所有权，不授权归档回填。
5. AI Native mapper 冻结计划后，由 `orchestration-runtime/.../llm_invocation.rs:138` 读取 node.config `/context_policy/auto_compact`；`internal_compaction/mod.rs:18` 默认 false。`control-plane/.../provider_invoker/internal_compaction.rs:94` 禁用即返回，启用后先从现有 input.messages clone/hash 再读取 checkpoint、验证当前 source/prefix；不是从 DB 按 token 任意捞一段。启用 host compaction 是明确配置的语义转换，应单列，不套用未经声明的透传保证。
6. `provider_invoker.rs:697` 校验 provider affinity；有 continuation affinity 时 `:726` 清除 canonical messages/system/tools，避免第二份历史伴随 native payload；`:765` 将 payload 的 wire_body/digest/size 放入 native_transport。`provider_execution.rs:234` 在真实 provider 选择边界应用该路径；`:900` 起的 completed_history 构建持久证明，供后续校验，不写回本次 input。
7. 生产插件 native builder（本机 `1flowbase-official-plugins/.../openai/src/lib.rs:3243`）clone transport.wire_body，路由 model、应用 store policy；`:3056` 选择 native 分支并设置 stream；`:4480` 包装 WS envelope。本轮以实际安装包 hash 及捕获 W 作最终运行证据，未将本机源码版本名当成已证明的二进制构建 commit。

## callback、cached replay 与合法恢复例外

`openai/response_dispatch.rs:190` 按 encoded/semantic/native 分别关联 callback；`native_tool_resume.rs:68` 的 repository 查询绑定 workspace/application/key/creator。响应不匹配、集合不完整、配置/历史证明不匹配会拒绝，不向错误上游发送。新 full context 通过 owned round 的有序 prefix proof（`compat/openai/history.rs:136`）区分续接和 exact replay。

`callback_resume.rs:199` 将 parked Native partial 送入 fenced inbox owner；`:562` 对已结算的 exact duplicate 返回既有 outcome，避免第二次 consumer。`callback_resume/inference_recovery.rs:194` 对已消费回执后追加新 context 识别为新调用；本轮 supplemental 真 API 特别验证这个分支，W 包含新 canary。

插件确实存在“恢复时构造 full body”的条件性代码，不能一概描述成永远只发 delta：`openai/src/lib.rs:2620` 校验相同 history scope、cursor provenance/epoch 和可物化 input；`:2724` 可从**被明确引用 response_id 的完成链**与当前 delta 生成一次 full-context retry，移除 cursor。`recovery.rs:364` 要求未提交语义输出、预算/期限允许，并限制 rebuild 次数；connection-bound cursor 有额外限制。这是引用上下文的条件性恢复设计，不是全会话 DB 捞历史，也不代表本轮正式 route 已发生该恢复。

核心矩阵的 4 个 transport-error 逻辑请求都实际只有 1 capture，主审逐 attempt 重算正文 exact 通过，错误为 provider_transport_unavailable；这不证明发生内部 retry。其 `http-full-fallback-view` 只是客户端主动提交 full view。

最后正式 cursor A/D 每版 4 请求：old root → 无 parent 的新 compaction root → 引用新 root 的 delta（注入 nested `previous_response_not_found` / `previous_response_id is no longer available`）→ 客户端主动 HTTP full fallback。delta 实际仅 **1 attempt、response.failed、0 内部 retry/rebuild**；导出 `1flowbase_provider_recovery` 为 `reason=semantic_failed, disposition=semantic_terminal, attempt=0`。receipt 没有单独序列化 binding，**不能把源码 ConnectionBound 分支当成本次失败的实测因果**。

后续 full fallback 成功、实际上游 carrier 为 HTTP；其完整 input 精确为 `newroot.input + mock 实际 newroot completed.output + delta`，没有 OLD canary。主审 Python 独立深比较全部 8 captures。它证明显式失败后客户端当前 view 的正确恢复，**不是内部恢复成功**；不删改正式 provenance 强行触发源码中另一分支。证据 `root-cursor-independent-audit.json` 与 `runs/root-cursor-{a1,d1}/`。

## D1 的真实缺陷与独立最小修复规范（未实施）

早先把 D1 seed 错拒定为“fixture 非法/需要 assistant 完成界标”的结论撤销。[官方 function-calling 示例](https://developers.openai.com/api/docs/guides/function-calling) 由主审通过 web 核验：`input_list += response.output` 后追加匹配的 `function_call_output`，直接下一次 `responses.create`，不要求先有后续 assistant。此工作区直接获取返回 403，失败记录保留于 handoff/，不将该网络限制解释为协议未定，也不重复抓取。

主审 `root-paired-a3` / `root-paired-d1` 使用 D1 **完全原请求**及最小 `[user, function_call, matching function_call_output]`，覆盖 HTTP/WS：每版 4 次无 assistant 均 `native_tool_output_unknown`、0 upstream（HTTP409，WS error）；每版 4 次严格只追加一个 assistant 的 control 均 200、1 upstream、完整 W 相等。原 request SHA256 `60cf10ed4f46a6ed5e9e4febc6df23360f594d42c9d892367f21b289d44b796d`，input SHA256 `daba8d3c94681db22c8308630a726f11de09ebb8e24ac1f41299e18c112da254`。报告中 `passed` 表示复现 checker 成功，**不表示合法输入兼容通过**。

根因：`ResponsesInputIndex` 虽记录 call/output 配对，`current_tool_output_positions` 仍从 last call 后列出该已配对 output；`native_tool_resume.rs:109–123` 在没有 owned callback 时，仅因缺少后续 assistant/compaction_trigger 就拒绝。原 unknown/assistant 启发式来自 `16be01694` / `a36d17321`，早于 939；A/D 原输入成对复现才是“既有缺陷、非 H/C/S 回归”的主要运行依据。

独立后续包的最小规范与边界：

- 无 parent（沿用 index 对 null 的既有无 cursor 语义）；每个被认定 current 的 output，都必须有本请求内先出现、同工具族、同 call_id 的唯一 call；拒绝重复、错族、逆序或缺配对。不需要后续 assistant。
- 结构闭合只证明这是可自包含采样，不赋予消费 callback 的权限。既有 owned pending/CAS、completed exact replay、跨 key、歧义与 frozen route/history 保护保持原 owner；不顺手删除旧启发式或扩大跨 session 的恢复授权。
- 可在既有索引/分类 owner 用 BTreeMap 实现 O(n log k) 时间、O(k) 空间；未知 items 保持 opaque，原 native body 不改写。
- 回归预期来自上述协议：原 D1 与最小形态 HTTP/WS 应成功且 W=M(I)；assistant-only control 仍成功；重复/错族/逆序/未知未配对输出失败；owned pending、并发 CAS、completed replay、跨 key/歧义仍保持现有行为。

本轮只交付该规范与已保存反例，不修改 classifier、插件或冻结 H/C/S 候选，不将“识别缺陷成功”折算为“兼容性已修复”。

## 93954ff 与后续修复的因果比较

939 `Merge beta into dev` 的父提交为 `9bab9f58d...` / `fdfe8b10e...`。相对第一父共 7 文件，产品改动集中在捕获 payload 复用与冻结 JSON Map 所有权移动。root 对下列风险 owner 的明确 12 文件集合做 blob 比较：11/12 与两个父完全相同；run_service.rs 的差异仅 cfg(test) module 引入。各故障机制另看当时源码；不根据“fix 在 merge 之后”推断由 merge 引入。

| 后续修复 | 可核对的具体根因与修正 | 对 939 的归类；候选状态 |
| --- | --- | --- |
| `570c0f9a4` | parked Native `Received` partial 被通用 settled replay matcher 错拒绝；改由 NativeToolResume/fenced reservation owner 接手 | 939 已有路径缺口；候选包含 |
| `68bfc6a7d` | callback 丢 predecessor carrier/recovery receipt；stale receipt 拒绝后没有 fault/settle lease，可能卡 inflight | 939 已有 carrier 重置及缺少 fault 分支；候选包含 |
| `04c35e423` | `68` 新增 receipt 读取根 snapshot.variable_pool，而实际恢复 waiting scope；改读 waiting_variable_pool | 精确错误由 68 新代码引入，不是 939；候选包含 |
| `b066a7cd8` | 冷回放把无 generation 的诊断 waiting 当当前终态；改以 original canonical waiting boundary 收尾 | 939 已有更宽的终态过滤缺口；新 boundary 实现不能反投射为 939 原样代码；候选包含 |
| `df98e4fa0` | DB/依赖错误被 map_err 折叠成未认证/404/无效映射；已 settle hard-expiry/hard-max tombstone 不能后继 | 939 已有；保留 typed error/source chain、允许有释放证明的 successor；候选包含 |
| `8f36fdc84` | cleanup budget 从入队算且被 TTL 耗尽，可能尚未首次 control dispatch 即超时；trajectory dedup 前复制大 output | 939 已有；首次 eligible dispatch 起算固定窗口，并移动已接纳 payload；候选包含 |
| `14cabc104` | 每次 semantic fact append 对 archive head 作无意义 UPDATE，产生 MVCC/WAL | 939 已有；只首次 null flow_run_id 绑定时更新，保留归属与 watermark；候选包含 |

全部 7 修复与 939 均为候选祖先，并复查当前修正分支仍在。H/C/S 没有改这些 owner 的业务条件：H 经过帧接收边界；C 只复用空 contribution 的 after digest；S 在真实 pool after_release 执行 flush → shrink_buffers → Ok(true)，仍由 SQLx ping。S 的影响面覆盖全部该 pool 归还，故用真实行值/事务/drain/error 恢复验证，不凭 diff 行数作保证。

基线 A 对应 `/home/taichuy/git/1flowbase_gateway` 的 `7f3d67a0dc75...`；该基线也包含上表全部修复。因此本轮是已经修复后的完整 gateway 基线与 H/C/S 的对照，不把较旧缺陷冒充本补丁回归。

另沿原生输入、分支、压缩和回执链检查了以下历史修复的实际 diff；它们也全部包含于 A/D：

| 提交 | 与透明性直接相关的故障类 / 规则 | 本轮对应证据 |
| --- | --- | --- |
| `dc021c6f8` | 内部摘要不再由 assistant 入口推断默认启用；改为显式 node opt-in，禁用前不读库；摘要本身也有预算 | 当前默认 false 的源码链；本轮使用关闭策略，未宣称覆盖启用后的全部 checkpoint 竞争 |
| `6489722e9` | user-role 工具回执可能遮蔽真正当前请求；compaction 选择/验证必须保留原用户意图和完整平行工具组 | 当前保护规则源码及既有测试存在；本轮 client opaque compaction 精确保留是不同路径，不偷换成该算法新测试 |
| `4176218fc` | 复制工具历史到另一已认证 session 时，不能误 claim 原回执或强套旧 prompt ID 的 digest | 新 session / 分支隔离和 full body 透明性；定向 CI 的 native-tool-resume 回归另计 |
| `a36d17321` | 已完成工具历史后的新采样不能被误判为 callback/recovery | 自造完整历史 seed、工具参数顺序、full fallback / 新 branch |
| `ee822121c` | 已消费回执加新 context 与 exact replay 必须分开，否则可能返回缓存结果而忽略新输入 | supplemental full-new-context 实际产生新 upstream 调用，包含新 canary |
| `d3ba5e3dd` | pending 工具结果携带刷新工具/完整 context 时，不能仅因配置变化拒绝已证明的新采样 | 当前有序 history proof / frozen model 检查源码；复杂刷新组合由现有 CI 回归覆盖，不扩大普通 wire 的字段豁免 |
| `e78804dbc` | nullable previous_response_id 应视为无 cursor，不能为了接受 null 重写 native input | 当前索引分支源码及本轮 CI nullable cursor 定向用例 |
| `c9154adf1` | 重放工具事实与取得 delivery claim 是两件事；没有 claim 时静默过滤会丢 native item | 现有冷回放/attachment 回归进入定向 CI；不能把协议重复呈现工具误称为服务器重复执行工具 |

这组历史表明此前问题不止一种“补回旧历史”：也包括误判 callback、忽略新 context、丢工具事实、压缩遗漏当前用户请求。检查必须同时拒绝多发、少发、错回缓存及错改顺序；仅检查请求长度或两个版本相等不够。

## 本轮矩阵与分母

| 正式行为组（每版） | 场景数 | 核心可观察证据 |
| --- | ---: | --- |
| seed、严格子集、另一会话、旧分支、相同 I 再发 | 5 | DB 有持久原生 items/manifests；全 payload 旧 canary 零出现；相同 I 的 W 原始 hash 相同 |
| summary+opaque、工具参数字符串/顺序 | 2 | encrypted_content 哈希、完整 input 深比较、arguments 内空格/转义/数值文本保留 |
| HTTP 两工具 seed、部分/另一半拒绝、完整集合、相同重复 | 5 | 两次部分 409/零上游；完整一次调用；重复相同 outcome/零上游 |
| HTTP 未知 parent、有效 parent/错误工具集合 | 2 | 404/409，零上游 |
| WS seed、有效 parent+delta、新分支、工具 seed/delta、未知 parent | 6 | 仅映射明确 parent；delta 完整保留；错误不调用上游 |
| HTTP full fallback view、重连完整 view | 2 | 同 session 仍只发送客户端当前 view |
| HTTP 传输失败、随后成功 | 2 | 故障保持可见、无伪造 completed；后续正常 |
| 长→短→opaque compact→长 | 4 | 真实旧大上下文样本，原 input 442 项 + 2 个完成界标；新请求 20,232,434 bytes |
| supplemental：跨 key、缓存回执后新 context、Native partial/duplicate/remaining、WS 失败后 HTTP/WS 当前 view 等 | 12 | Native partial 两次零上游，完整后一次调用，两结果各一次；新 context 真到达上游 |
| 合计 | **40 × 2 = 80** | A/D 正式上游请求 **62**；普通透传 W=M(I) **56**；Native 完整续接 **2**；受控 transport error **4** |

原大请求旧 20,232,417 bytes 与新 20,232,434 bytes 的差是本轮 assistant 完成界标新增 `"type":"message",` 的 17 bytes。没有改写旧样本数据、旧计数或旧报告；本轮报告准确采用新请求大小。正式长请求共 4 次，不是重新进行内存性能试验。

## 二进制、资源与失败记录

- A：原 baseline `7f3d67a0dc75...` 与构建 marker `5ccea15084eb...` 的 api/ 无差异；实际 binary SHA256 `0b20528ae35d25f88c224d0b8171105475749b66bdf6c8ca8e79b59a5badc57a`。
- D：生产构建 `76579b5e759774fbe0b9e18906c87568d3ee1f75`；测试修正 `34e43884abbb99fd8c579b4692392b5dfebac32c`；候选 runtime 源码不漂移，差异仅 PG 测试 URL fallback。binary SHA256 `470e407828c17127d844cbb1ce5a819e1f7954d1078a1506f820eba6067e5f0b`。
- 所有实际 API 使用同一签名生产 `openai@0.2.80` 包，SHA256 `be2f68ed920e0c82685315096a296b31fffcee1402b3b16484b1cd8064a6f79d`；包内 worker hash、签名 metadata 单列保存。没有改插件或向真实模型发送 raw。
- 沿用原 7GiB / 6CPU cgroup，task memory 8GB 上限、单重作业、首个 host CPU >60% 即停自有 scope。A/D 顺序运行；核心正式 A1/D2/A2/D3 回放均无资源中断，最大聚合采样 memory 1,598,468,096 bytes。原任务历史峰值与本轮峰值分别报告，不覆盖旧事实。
- 新测试本地构建尝试单列：第一次 narrow cache lookup 漏 proc-macro .so，未启动编译；第二次触发 CPU guard 后立刻停止；第三次单 CPU、不再超限，但缺完整 rlib/extern 闭包，编译失败、零业务测试。停止本地构建路径，转现有 Actions 定向任务；没有全 API 本地重编、补复制依赖缓存或把编译失败记为业务失败/通过。
- 7600/PID3513173、7800/PID3845384 未发 signal/restart/attach/load。仅创建/删除自有临时 DB 与端口。正式各轮 session/API/mock/DB cleanup 均 true；最终独立资源复核见下方。

## 新定向 CI 与独立 QA

现有 `gateway-target-handoff.yml`，仅 `regression` job、`verify_callback_attachment=true`，绑定冻结 `b59f5058`。临时审计引用 `codex/gateway-transparency-audit-20261031` 仅用于取证，没有 merge/deploy。新 [run 41179723119](https://github.com/taichuy/1flowbase/actions/runs/41179723119) 完成成功；原始 artifact 的 source-sha.txt 与候选精确相同。job 07:13:07–07:34:18 UTC；不是本地构建，也不是原旧 CI 的重复引用。

| 实际执行组 | 通过数 |
| --- | ---: |
| registry / transport lifecycle | 44 + 103 |
| callback admission / Native tool / prepared / execution / carrier | 16 + 38 + 4 + 3 + 3 |
| callback live/durable forwarding / committed index | 96 + 3 |
| 真实 PG callback lifecycle | 37 |
| callback contracts | 66 |
| runtime event retirement / catalogue ownership | 54 + 5 |
| nullable cursor | 3 |
| 合计 | **475**，475 个不同的完整测试名、0 失败 |

PG 唯一 ignored 为 `callback_resume_context_keeps_200_large_snapshots_out_of_process_history`，标记 `explicit 200 callback / 3 MiB RSS regression gate`。未执行，不算通过；本轮不重复该内存压力实验。CI 包含 cold replay canonical boundary、ACKed/uncertain exact tool replay、partial Native compiled-binding 单 owner、scope carrier、stale receipt、hard-max successor、cleanup dispatch budget、配置刷新/完整历史证明。其他大 lane 和插件 build 均 skipped，不把它们计为覆盖。

仅通过 zip range 取回 17 个日志/身份成员，网络取回 741,501 bytes；没有下载 artifact 中的 575,398,712-byte API executable，也没有复制依赖缓存。机器摘要 `ci/summary.json` 与 `ci/artifacts/manifest.json`，全部原测试日志可逐名复算。

fresh 独立 QA 不调用本轮 oracle.cjs 作为判断依据，直接复算完整 body、DB public/upstream 身份、原始响应、input/opaque/arguments 和负控：80 正式场景/62 calls 成立，56 普通 W=M(I)、2 Native 完整 inbox、4 预期 transport failures；逐值核验 550 个 opaque 字符串、10 个 arguments 字符串、8 条有持久证据的 parent 映射。12 个捕获副本负控另计，不冒充生产故障。

QA 第一次 checker 在首个 host CPU >60% 样本被 guard 停止，未计 wire 通过；观察到 12.33/9.17/12.99% 的稳定下降采样后，仅单 CPU / 1GiB AS 补做一次完成 wire 核验。其后两项 checker 将新增共享目录误算进冻结 inventory，原误报保留，按明确冻结集合补结算，无须重跑 wire。最后一次廉价 settlement 自有 scope 的 `cpu.max=25000 100000`，三条 QA scope 均 inactive。

本线程最终自有资源结算：**12 scopes inactive、6 exact DB 不存在、5 API PID 不存在、5 scratch 不存在**。本轮原始 scope 的 task memory 最大采样 2,484,244,480 bytes（窄编译尝试，含 cgroup charged pages），API 回放最大 1,598,468,096 bytes；自有 CPU guard 中断 2 次（一次窄编译、一次 QA），OOM 0。最终 QA 判定和可复算结果见 `packet-t/qa/`。

## 主审授权补证与最终资源总账

root-paired-* 与 root-cursor-* 均为本任务主审依据用户授权执行的独立诊断。来源已由主审通过本地 Codex 指令 18/19/20 明确确认，旧“来源未确认”的判断在此更正；原观察和失败记录仍保留。成对工具历史与 cursor 组维持独立分母，不混入原 80/62、72 或 475。

主审 root-resource-final-audit.json 合并核心执行、独立 QA 和主审补证，共 **18 scopes**；整机 CPU 保护停止共 **3 次**：lifecycle 68.48%、QA 64.19%、root 初次启动 67.33%，每次只有首个越线样本。全部 OOM=0；scope memory 最大 2,484,244,480 bytes，全部 scope 已 inactive。这个数是整个 scope 的 cgroup 用量，不是 API RSS。原 12-scope / 2-stop 是核心执行与 QA 的子集，不能当作全任务总量。

最后 cursor A/D 分别 16.14/19.14 s、scope 峰值 637,620,224 / 735,309,824 bytes、整机 CPU 42.23% / 57.12%，没有新增 guard stop。具体请求与正式 receipt 见 root-cursor-independent-audit.json；它证明实际正文和明确故障，不表示内部 retry 成功。

最终按报告和既有 cleanup 清单逐一只读核对 **12 个自有 DB、11 个 API PID、11 个 fixture scratch** 均不存在。原 root-paired-a 保护停止留下的自有 scratch 经确认其 DB/PID/scope 均已释放后清理，日志先保存在该 run 的 private 证据目录，失败记录未改写；收据为 root-failed-scratch-cleanup.json。7600 观察进程没有 signal、restart、attach 或测试灌流。

主审最终综合验收见 root-final-acceptance.json；原独立 QA 记录和运行原始文件保持原状。冻结 H/C/S head 保持 b59，不新增产品改动、不提交改变冻结 head、不合并；审计文档保留可审阅工作区状态。

## 证据结算与限制

本机 gitignored 根：`tmp/test-governance/gateway-memory-deep/packet-t/`。

- status.json / root-final-acceptance.json：最终机器可读范围、源码身份、分组结算、开放缺陷、失败/中断与清理账本。
- `source-identity.json` / `source-audit.json`：候选 API 文件 hash、祖先关系、父 blob 比较、二进制源码 guard、Codex 与插件身份。
- `ci/transparency-history.json` 与对应 diff：另外 8 个透明性历史修复的原始源码/测试比较；`ci/baseline-ancestry.json` 固定 A 已包含 939 后修复。
- `runs/{a1,d2,a2,d3}/report.json`：正式案例、逐条 capture index、映射与 provider body hash。各自 `private/` 保存 I、W、client raw、Native response 和三张运行/回执表导出；只留本机，不提交、不打印或外发真实内容。
- `runs/d1/`：首次 14 场景试运行的全部原记录；`d3-native-status-adjudication.json`：两个公开 waiting 状态的 raw 复核，不改 D3 原断言结果。
- `components/results.json` 与逐测试日志：本轮 72 个实际执行；`lifecycle/` 与相邻原资源 guard records：未完成本地窄构建的真实失败证据。
- `ci/summary.json` / `ci/artifacts/`：新 CI 475 个非零实际通过、1 ignored、source SHA 与原始 logs；未下载的 API artifact 单列。
- `qa/QA.md` / `qa/results.json`：fresh QA 最终判定；`qa/independent-results.json` 保留最初 checker inventory 误报，`qa/independent-settlement.json` 按冻结集合补结算；`cleanup.json` 是本线程最终自有资源复核。
- `../raw/packet-t-*/{command.json,command.log,resources.csv}`：每个自有 scope 的实际命令、退出码、CPU/memory、停止原因。

WS 的 `*-request.json` 保存 envelope 内部 body；客户端发送函数仅添加 `type=response.create`。实际 W 来自 mock 收到的 HTTP bytes / WS message。该记录层级显式声明，不将 WS body 文件误称为包含 envelope 的原始网络帧。

旧 packet-s 的 24 次 full-request A/D、六个 PG 事务语义、旧 Luna、既有 CI4 WS 用例及 9 页 PDF 均保持原状。旧 24 次只有完整请求相同输入对照的结论，本轮新增部分/压缩/分支/回执/重连证据独立计数。

当前限制：未穷举所有 provider/插件、跨协议映射、启用内部 compaction 的所有 checkpoint 竞争、所有连接绑定 cursor 的物理恢复组合；未进行生产灌流、生产重启、真实模型复跑或全仓门禁。对这些范围不作“完整功能不可能再报错”的承诺。主审补证来源已确认；已发现的合法工具历史错拒作为开放产品缺陷单列，修复规范尚未实施。
