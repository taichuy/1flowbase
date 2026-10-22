# Responses 跨协议兼容

网关按“映射协议 → AI Native → 供应商”分工。兼容以保留可表达的语义为目标，不承诺所有供应商具备 Responses 的全部能力。

## 通知与工具回执

`function_call_output` 的 `call_id` 缺失或为 `null` 时，它是独立外部上下文。映射层保留完整 JSON、`name`、`namespace` 和扩展字段；AI Native 使用 typed external context 保存，统一投影为标明来源的 user 级数据消息。它不会成为 system/developer 指令，也不会伪造工具调用 ID。消息中的来源和内容仍是外部数据，不应被模型当成可信指令。

非空 `call_id` 才参与工具回执关联；空字符串及错误类型被拒绝。完整历史和 `previous_response_id` 增量续接都保留通知与回执顺序，并继续检查调用所属 scope、完整回执集合、重复回执和历史证明。通知不完成或重开工具任务。


Chat 类供应商要求 assistant 工具调用后紧邻完整工具回执集合。共享投影将该集合内交错到达的通知放到回执组后的外部上下文消息，并用原始消息顺序和真实回执引用记录时间线；canonical 历史、真实回执内容和 ID 不变。未完成或关联非法的工具轮不会被这一投影修补。

## 传输选择与表达提示

通知及仅包含 `text.verbosity` 的请求同时保留语义表示和原始 Responses body。选定供应商同时具备 `responses.native_passthrough` 与 `responses.native_output.v1` 时优先使用原始请求，否则走语义表示。必须使用原生协议的字段以及已有供应商 continuation affinity 不允许降级。恢复、输出及历史元数据依据实际选定的传输模式。

`text.verbosity` 的 `low`、`medium`、`high` 在语义路径转换为本次请求的表达长度提示，并在 translation report 中记录 Normalized。提示的遵循程度取决于模型，不等价于供应商的原生参数。未知 `text` 扩展或 `text.format` 仍要求原生能力，非法 verbosity 值被拒绝。

## 供应商边界

DeepSeek 插件只负责 Native 工具选择到 Chat Completions 的转换：`auto`、`none`、`required` 转为字符串，指定工具转为 `function` 对象。通知投影、子进程通信及恢复状态机由底座负责，插件无需实现 Responses 仿真。

普通文本、标准函数工具及上述通知可以通过这条兼容路径工作。这不代表完整 Codex 功能兼容：自定义 grammar 工具、严格工具 schema、原生托管能力和无法跨供应商表达的私有推理仍遵循已有能力检查，不能靠字段丢弃模拟成功。

## 协议恢复持久化与请求准入

HTTP / WebSocket 的请求准入由协议适配层拥有；持久化协议上下文与 continuation 原生历史由 `PgProviderProtocolCapsuleStore` 保存完整内容，不能再猜测客户端载荷或宿主资源条件，添加更小的业务容量上限。已移除独立的 2 MiB capsule 拒绝条件，超过旧阈值的上下文和历史仍可加密保存、重启读取和原子消费。物理类型表示、实际数据库 / I/O 失败继续显式传播，不吞错或截断历史。

此次修复保留 flow-run / capsule-kind / slot-key 的 AEAD 身份绑定、digest / size 完整性验证、已有过期策略与调用所属权限；数据容量和生命周期是独立维度。真实 PostgreSQL 回归对超过旧阈值的两种载荷逐值恢复，并检查密文、locator 与唯一消费语义。

## 请求与交接大小

Gateway 的兼容协议、Native HTTP（含文件输入）与 WebSocket 不使用内置的正文、消息或帧字节上限。Host 与共享 SDK 的 stdio 保留完整 NDJSON 消息；写窗口与确认窗口只调度传输，大消息可独占窗口继续推进，不限制累计响应或返回正文。同步供应商事件回调不能等待同一执行线程的 writer，已接纳事件保留至输出完成；这不承诺任意大小请求的内存成本恒定。身份、取消、协议校验及真实 I/O 错误仍由原 owner 负责。

宿主运行事件流沿用现有 required delivery 分类：业务增量、输出项与终态不受诊断数据的单条或累计容量限制，也不会为诊断缓存腾挪而被驱逐。实时消费者继续使用背压；完成且确认落库的运行事件继续按已有约 5 分钟窗口回收。诊断数据的保留与展示预算保持独立，不改变持久历史和数据库真值。

## 内容投影与历史归属

Chat 入口先完整校验消息，再按原消息顺序生成翻译回执与 Native 内容。每条消息只构造一次内容投影；最后一条 user 的已构造内容用于 query，媒体历史沿用同一投影。assistant 的 reasoning 块位于可见内容之前，已有媒体块由当前 owner 移动到最终历史对象，避免通过 JSON 反查后再次复制。校验阶段与投影阶段分别拥有错误优先级和内容转换职责。

Anthropic 消息的结构与支持范围由内容块校验器判定；校验成功后，query / history 映射器才构造所需文本，校验阶段不再生成随后丢弃的文本副本。错误优先级与有序翻译回执保持不变。未知字段的匿名路径只表达父路径和字段数量，不读取或排序字段名；根字段处理中按名称决定保留或丢弃的排序仍保留，以维持匿名序号与处理结果的对应关系。

助手持久历史由 `control-plane` 的 `run_service/conversation_history` 投影：在已有 `AssistantConversationNativeMessage` 上保留完整且未重复的工具调用，再通过现有 `into_value()` 交接 JSON。工具调用原文和扩展字段保持不变；真实结果完成对应 pending 调用，孤立或重复结果按既有规则丢弃，未完成调用在原有边界补充中断结果，不重新执行工具。持久化、身份及查询权限继续归原 owner。
