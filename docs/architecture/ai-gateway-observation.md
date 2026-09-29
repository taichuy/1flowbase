# AI Gateway 翻译与执行证据

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
