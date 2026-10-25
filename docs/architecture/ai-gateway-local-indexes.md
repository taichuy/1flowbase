# AI Gateway 调用内索引与顺序保持

本页限定两个计算 owner：兼容协议流的已转发 committed event 集合，以及 Assistant Native 历史中的待匹配 tool call。
索引只活在当前转发流或单次历史转换中，不引入跨调用缓存、数据库表、容量上限或新的依赖。
它不是整个 Gateway 的端到端正确性证明。

## 已转发事件

`api-server/.../compat_sse/event_forwarding.rs` 以 `(event_type, payload)` 的原有相等关系识别已转发事实。
`ForwardedCommittedEvents` 使用标准库 `HashMap<String, HashSet<Value>>`；哈希只加速定位，命中后仍由完整 JSON `Eq` 决定相等，不使用摘要、call_id 或近似匹配代替正文相等。
不同事件类型、不同字段类型或不同正文保持独立；JSON 对象字段排列不改变原有 JSON 相等语义。

设旧有序表为 L，新索引为 H，不变量为：

    H[t] = { p | (t, p) 在 L 中 }

初始二者为空。查询返回同一成员资格；插入同一对保持不变量。
外部输出顺序由原事件遍历、游标与发送过程决定，绝不遍历 HashSet 生成输出。
已有 ACK、receipt、release、游标与终态分支位置不因索引变化而移动。

设一次正文哈希成本为 H(P)、完整相等比较最坏成本为 E(P)。原表处理 n 个唯一事件及回声最坏为 O(n²·E(P))；索引通常散列条件下为期望 O(n·H(P))，哈希冲突仍会执行完整相等比较。
当前 serde_json 启用 preserve_order，JSON 对象哈希会临时收集并排序 key，因此 H(P) 包含正文遍历与各对象的 key 排序成本，不承诺对任意 JSON 严格按字节线性。集合仍保留全部正文，空间为 O(n·P)，另有桶与哈希临时空间；少量大正文可能增加 CPU，需保留相同输入的反例对照。

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
