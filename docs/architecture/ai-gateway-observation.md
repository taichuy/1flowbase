# AI Gateway 翻译与执行证据

[English](ai-gateway-observation.en.md)

## 职责

执行保持 `映射协议层 ⇄ AI Native ⇄ 供应商插件 ⇄ 上游`。

- 映射协议层解析客户端协议并投影响应；客户端轨迹记录实际请求与响应，不从 Native 反推原文。
- AI Native 承载统一执行语义。在调用插件前观察实际输入，在插件返回时观察实际结果和错误。快照证明交给插件的内容，不冒充供应商原始报文。
- 供应商插件适配 Native 与上游协议并负责通信，不承担轨迹存储或查询。
- 日志基础设施通过现有 runtime event repository 保存调用事实和正文定位，节点正文与客户端请求字节分别归档。可归并文本增量仍实时发送，完整 item、工具、usage、错误和中断证据在语义边界持久化。观察失败不能改变执行结果、客户端协议、工作流状态或费用结算。

## 必要记录

保留实际模型、参数、上下文、工具定义、实际返回结果，以及 run/node/invocation/attempt 关联、错误与采集完整性。客户端未暴露的内部调用仍有独立证据。

不记录翻译函数的临时对象或转换步骤，不在读日志时重新运行翻译或工作流。用量和费用的结算真值继续归已有账本；Native 结果中的供应商用量只是收到的证据，不形成第二套结算。

## 同调用内正文复用

请求、回复快照使用唯一 UUID step key。工具结果若已包含在请求快照内，工具调用若与回复快照中的值完全相等，子步骤只存 `body_ref = { step_key, pointer }`。pointer 使用 JSON Pointer；相同 call_id 不足以证明内容相同。请求被截断或快照未获准进入采集队列时，仍保存能够采集到的独立工具正文。

回复正文使用 `body_format = native_reply_v2` 时，外层 `final_content` 与 `result.final_content` 完全相等才省去后者。后端读取时恢复既有详情结构，不要求前端识别存储格式。未知扩展字段和不同的流/终态证据保留。

`body_ref` 只在同 run、node、invocation、attempt 的 Native 索引内解析，且只能引用单版本、不再引用其他记录的快照。不使用历史可变的 request/reply 固定 key，不跨调用解析语义引用，不构造差量链。

列表只读窄索引。打开详情才读取快照；无引用的历史记录保持原有读取方式。引用缺失、指针不存在、源快照被覆盖或形成引用链时显式拒绝，不重建、不串读、不写回历史。相关事实随运行日志共同保留和清理；未来若增加单事件清理，必须同时处理引用，不能只删源正文。

## 边界

物理正文复用与 `body_ref` 的语义关联分别处理。Native / 供应商观察正文可复用同 application / scope 的不可变 canonical 内容；SHA-256 命中后再验证完整 JSON 值相等。Native model_call 的原始输入 String 另用不可变 manifest、item 与有序引用保存：JSON 数组中的完整 item 共享精确原始字节，literal spans 保留空白、字段顺序、数字写法与转义，重复 occurrence 仍独立出现。读取核验归属、引用、字节长度和整串 SHA-256 后恢复完整 String；未知或无法无损拆分的形状沿用 canonical。每个观察点的 run、node、invocation、attempt、来源和格式仍独立保留。相同正文不表示相同调用，也不用于执行幂等或推断客户端字节；不做跨租户复用或相似匹配。

由观察归档首次创建的 canonical 内容具有明确所有权标记；最后一个 durable 引用删除后，以延迟事务触发器校验 runtime event、client section、context、recovery、legacy shadow 引用并回收。复用原有 canonical 内容时沿用其原有生命周期。Native manifest 在最后一个 event 引用删除后回收，item 在最后一个 manifest 引用删除后回收；共享写入和回收使用相同 application 锁。运行删除不保留新的无主观察正文，选择性备份按 application owner 保存全部新表。

节点目录仅存身份和运行状态。客户端 Step / Section 新写入直接保存发生目录，不再各写一个 runtime event；Integrity、NodeLink、ResponseLink 仍保存真实事件。全 run 的 durable sequence 高水位同时覆盖事件与目录，旧 numeric cursor 不重编号。section 复用同 application / scope 的精确 canonical 原值；parameters / result 只有与已恢复 overview 的指定 child 完全相等时才保存 locator。普通 timing 复用 occurrence 的 observed_at，特殊形状保存完整值。后端完整恢复 originals 后在 Rust 选择 child，原 DTO 保持一致。

客户端 request 独立保存实际字节与版本化紧凑帧目录，节点仅关联请求；流式帧不再各占一个 runtime event。新 part 使用 zlib 无损压缩，校验原长度、目录和 SHA-256；逐 part 解码后按原帧分页，保留精确 bytes、时间、kind、顺序和 sequence。v0 reader 继续读取旧 wire / legacy 格式，SQL legacy 引用所指 part 不转换。写入背压、head 锁和 commit 后完成回执保证已接纳字节持久化。

历史维护只接受显式 run allowlist，经真实 reader 恢复和完整值 / 帧核验后切换引用，可重入；启动与请求路径不自动搬迁。旧事件身份及游标保留，Step 历史 revision 保留原 envelope，可验证的 Section 正文改为小引用。NUL 等不能安全转换的历史 Section 保留旧正文和 reader，记录跳过状态以继续处理后续批次。新数据仍走无损 originals 路径；长期上下文不放入 ephemeral。

预绑定采集在最终 owner EOF、已接纳帧排空后关闭绑定；无运行归属时由 repository 显式清理 head/parts。清理失败返回完成错误并保留字节，排空期间的晚绑定继续保留轨迹。此收尾不以 TTL 猜测运行状态；进程崩溃前未完成的采集仍可能留下待调查记录。

删除 UI Tab 不会停止采集；UI 展示策略与证据保留策略分别决定。

## 客户端请求与工作流内部事件

用户视图分别称为“客户端请求”和“工作流内部事件”。前者保持映射协议原文；后者按LLM调用、尝试组织实际系统输入、上下文、工具、配置与输出。用途取自实际 generate:false、被接纳的本轮工具resume或真实operation，不以空响应、历史工具消息或握手标签推断。

每轮HTTP/WS采集ID通过宿主私有、消费一次的 WorkflowObservationContext 传到当前执行段。它不从公共JSON反序列化、不进入插件请求、用户变量或checkpoint。Native事件携带当前 trigger_request_id 与独立的 context_flow_run_id/context_response_id。前序来源必须先通过现有鉴权和响应上下文解析。

两端采集可异步到达，已有Native事件窄索引保存明确身份；读取时在同运行解析trigger，在同应用且明确前序运行的emitted响应索引内解析context。不为先后顺序新增外键依赖，不按节点/时间猜测或回填历史；不存在的来源不生成可导航链接。

两个列表支持request过滤与首屏focus定位，后续沿普通cursor分页。详情source链接打开具体请求，客户端请求打开对应调用列表；一对多关系明确呈现。查询仍经现有console授权入口，跨运行来源同样按目标应用验证。前端保留返回时的筛选、选择和滚动状态。原有工作流节点树、输入/处理/输出与Resume时间线保持职责。

## AI Native 调用轨迹与协议诊断

AI Gateway 的映射协议层将请求接入 AI Native 统一契约；供应商插件负责将 Native 翻译为上游协议及通讯。日志不迁移这一翻译职责。

- 主轨迹在 Native 调用边界记录安全输入快照、标准输出/工具请求、调用结果及供应商扩展信息。工具请求与提交的工具结果不证明工具已执行；真实执行沿工作流执行记录关联。
- 原始供应商协议是可选诊断证据，不是主轨迹成立的前提。Native 详情不能标成供应商实际报文，未知供应商元信息只保留为扩展，不据此推断插件内部重试或执行成功。
- `FLOWBASE_PROVIDER_PROTOCOL_CAPTURE=1` 在 API 服务端显式启用原始采集，默认关闭；启用后重启服务生效。宿主仅在具备原始观测 sink 的调用上向插件协商能力，客户端输入不能打开该能力。插件不支持此可选能力时，Native 主轨迹仍可用。
- 供应商流计时默认仅保留每次 attempt 的精确摘要（事件数、字节总量、事件种类计数、入站时间范围、最大写入延迟），不保留逐事件计时数组。执行前在 API 服务端设置 `FLOWBASE_PROVIDER_STREAM_TIMING_CAPTURE=1` 并重启，才完整保留五字段计时明细；调用开始时固定模式，不截断记录。摘要位于 `metrics.attempts[].provider_stream_timing_summary`，诊断明细位于同级 `provider_stream_timing`；这只改变性能诊断明细，不改变语义、工具、恢复或投递事实。
- 语义记录与原始协议分别计算采集完整性；一次失败调用可以被完整记录，缺少原始证据不会降低完整的 Native 轨迹。旁路丢失、写失败、取消和超限必须如实表达。
- 列表只读取摘要；选择步骤获取 `view=semantic` 的 Native 详情或历史投影详情，显式打开协议证据才读取 `view=protocol`。Native 步骤的原始证据按 invocation/attempt 关联，显示调用级范围，不伪造逐步骤网络对应关系。
- 历史供应商投影保留 `supplier_protocol` 来源；新 Native 记录标明 `ai_native`。不从旧协议反向伪造 Native 历史，不在 GET 路径重建投影。

## 日志读模型与观察结果收敛

日志投影收敛不表示工作流或计费已经完成。`waiting_callback` 的一次等待阶段有 5 分钟投影期限；真实执行进展（进入等待、成员运行或调用/回调次数变化）开启新的等待阶段，重复 projector 写入、用量修正和页面读取不能延长期限。该期限只管理日志读模型，不取消执行、不伪造工具回调，也不删除恢复证据。

已收敛投影优先读取最后客户端文本，其次读取非冲突的供应商完整 assistant message，最后使用已有 `final_output`。有真实文本时标记 `observed_output`；没有时使用 `timeout` 来源与 `Timeout` 占位。后端 overview 将 `persisted_answer` / `provider_output_item` 对应的业务答复放入 assistant 消息，超时占位仍是独立投影输出；不把供应商片段或占位当成真实答复。overview 为返回列表重新分配唯一顺序，避免多个运行的局部序号碰撞。

费用读取已结束成员的持久费用快照；快照缺失时汇总已有 cost ledger。用量优先汇总已有 usage ledger，没有 ledger 记录才读取成员摘要。每次从事实重新计算，不向旧投影累加；缺失观察保持未知，不补零。此过程只更新日志投影，不写回执行状态、callback、usage 或 cost ledger，不形成第二套结算真值。

实现入口：[投影维护迁移](../../api/crates/storage/durable/postgres/migrations/20261010120000_observed_log_projection_settlement.sql)、[overview reader](../../api/crates/storage/durable/postgres/src/orchestration_runtime_repository/agent_logs/reads.rs)。

任务聚合与单次尝试分别保留状态：只要仍有活跃成员，任务 outcome 为 `in_progress`，即使先前已有答复也不宣称完成。全部成员终结后，任务状态优先取最新 generate 尝试；无 generate 时按其他调用的最新尝试回退，避免计数或压缩调用掩盖生成结果。失败尝试仍保留在源运行和轨迹中，已知费用跨全部尝试汇总，全部未知仍为 NULL；不把重试成功当成先前调用未发生。

实现入口：[终态任务投影](../../api/crates/storage/durable/postgres/migrations/20261010130000_project_terminal_task_attempt_outcome.sql)。
