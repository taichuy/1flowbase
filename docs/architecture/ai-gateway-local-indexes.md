# AI Gateway 调用内索引与顺序保持

本页限定两个计算 owner：兼容协议流的已转发 committed event 集合，以及 Assistant Native 历史中的待匹配 tool call。
索引只活在当前转发流或单次历史转换中，不引入跨调用缓存、数据库表、容量上限或新的依赖。
它不是整个 Gateway 的端到端正确性证明。

## 已转发事件

`api-server/.../compat_sse/event_forwarding.rs` 以 `(event_type, payload)` 的原有相等关系识别已转发事实。
`ForwardedCommittedEvents` 先按事件类型分桶，再按正文自身的字符串身份定位候选集合：依次读取 `call_id`、`item.call_id`、`item.id`，没有字符串身份时保留完整 `Value` 的 HashSet。身份只缩小比较范围，桶内仍以完整 JSON `Eq` 判断，不使用摘要或 call_id 代替正文相等。
不同事件类型、不同字段类型或不同正文保持独立；JSON 对象字段排列不改变原有 JSON 相等语义。

令上述身份投影为 f。完整相等必然有相同投影，即 p=q ⇒ f(p)=f(q)，反向不成立。同 ID 的不同正文可以共存，空字符串不 trim，非字符串继续查下一路径；不从 envelope 的 sequence/generation 推导身份。每个新 ID 的 Vec 只分配一项容量，需要保存不同正文时再增长。

设旧有序表为 L，新索引各分区的并集为 H，不变量为：

    H[t] = { p | (t, p) 在 L 中 }

初始二者为空。查询返回同一成员资格；插入同一对保持不变量。
外部输出顺序由原事件遍历、游标与发送过程决定，绝不遍历 HashSet 生成输出。
已有 ACK、receipt、release、游标与终态分支位置不因索引变化而移动。

原表处理 n 个唯一事件及回声，最坏需要 O(n²) 次完整相等比较；不同正文可能很早就不相等，不能把每次比较都当成遍历全部正文。带身份的索引通常只哈希事件类型与 ID，再扫描对应桶；唯一 ID 时查找为期望 O(1)，匹配相等大正文仍需完整 Eq，插入仍复制完整正文。同 ID 对应 k 个不同正文时，最多扫描该桶的 k 项，极端分布仍可能退化为平方比较。

没有字符串身份的后备集合继续哈希完整正文。当前 serde_json 启用 preserve_order，JSON 对象哈希会临时收集并排序 key，成本包含正文遍历和字段排序。保留完整正文的空间仍为 O(n·P)，另有桶、ID 和临时哈希空间；小正文的索引固定开销、无 ID 大正文的后备成本都必须在资源对照中保留，不宣称所有输入都更快。

## 批量投递

批次仍按 `(sequence, event_id)` 排序。所有权通过 `mem::take(deliveries).into_iter()` 逐项转移，替代反复 `remove(0)`。
处理 k 项之后，迭代器剩余内容严格等于旧向量删除 k 个首项后的后缀；排序、重复项处理与输出顺序不变。
发送失败时，当前项由 receipt Drop 释放，尚未生成 receipt 的后缀仍显式 release；已发送前缀由下游 writer 持有。
内部入队不产生 ACK，只有原协议写入/投影回执有权结算。

这消除了 n(n-1)/2 次元素搬移；不消除排序和其他 replay claim 匹配成本。
整个任务被强制销毁时的既有租约恢复行为不由此改写；不能把普通断连处理外推为任意取消都同步释放。

## Assistant Native 历史

`control-plane/.../run_service/conversation_history.rs` 保持已有历史语义：
完整调用要求非空且 trim 后有效的 id/name，arguments 只要求存在；保留第一个完整调用的原 JSON。
结果的 tool_call_id 按原精确值匹配，不新加 trim。孤儿和重复结果仍丢弃。
在非 tool 消息边界与输入结束时，为未完成调用按原出现顺序补中断错误结果，不执行或重试工具。

新待处理表为 `HashMap<id, (ordinal, name)>`。
每个 assistant 调用列表开始前，上批待处理已清空；插入期间没有结果移除，因此成功插入时的 len 给出唯一递增 ordinal。
随后工具结果只删除条目；flush 按 ordinal 排序，等于旧向量保持顺序删除后的剩余条目。
由消息序列归纳，新旧输出内容与序列化顺序相等。

对于 m 个调用、r 个结果、u 个中断调用，匹配由原最坏平方级扫描改为期望 O(m+r)，flush 为 O(u log u)，另计输入输出字节处理成本。
这是已持久化 Assistant 历史恢复的优化，不宣称覆盖所有公共协议请求路径。

## 验收职责

生产单元测试保留 JSON 相等、流隔离、重复、空白、缺失字段与历史边界的反例。
开发试验使用原函数摘取的 SHA256、旧新差分、有限穷举及真实 Tokio 通道的 receipt 边界 fixture；fixture 不替代 PostgreSQL 和完整 API 集成验收。
微基准固定同一输入与依赖，分开比较最近发版、当前基线和候选；函数耗时不冒充网关延迟或吞吐。
