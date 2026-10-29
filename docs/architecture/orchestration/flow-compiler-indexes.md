# FlowCompiler 编译期索引

`orchestration-runtime::compiler` 独占图编译与变量聚合声明解析。索引只在一次编译中借用输入；不持久化，不新增依赖或业务容量上限，不改变运行态状态或权限。

## 图关系

`topology.rs` 将输入边完整保留为 `CompiledEdge`，包括重复边、输入顺序与 handle。Kahn 排序的 adjacency 与 indegree 仍按多重边计数，初始队列按节点声明顺序构造。

单个 `HashSet<(&str, &str)>` 仅判断 source/target 关系首次出现，再向 dependency/downstream Vec 写入；不通过集合迭代生成输出。关系 Vec 仍按输入节点 rank 排序。哈希键借用原 JSON 文本，不复制节点名。

若 E 是输入边数、U 是唯一节点对数、V 是节点数，关系判重由最坏 `O(Σ indegree² + Σ outdegree²)`（稠密图 `O(V³)`）改为期望 `O(E)` 哈希查询，另保留原有 BTreeMap 查找与关系排序成本。索引增加 `O(U)` 临时空间；不是内存缩减方案。字符串哈希成本还取决于标识符字节长度。

## 聚合变量声明

声明解析归 `variable_aggregator_contract.rs`，不放在图拓扑 owner。首次实际聚合候选访问时建立 `DeclarationIndex`，同次编译的后续候选复用。没有候选时不建索引。

run-level 使用完整分段 selector 作键，conversation 使用声明 name 作键。值保存首项声明及重复标记；建索引不验证声明，只有引用时才报告错误。保留重复声明、缺失类型、非法 selector 长度与候选顺序的诊断优先级。节点输出仍消费既有 resolver。

R/D 分别是 run-level/conversation 声明数、C 是查询数，重复线性扫描从 `O(C(R+D))` 改为期望 `O(R+D+C)`（另计 selector 哈希字节成本），增加 `O(R+D)` 临时索引空间。小输入存在构建常数开销，不能推导所有流程都加速。

## 验证与门禁归属

业务 fixture 覆盖多 handle 重复边、节点重排、完整依赖/下游及拓扑顺序、未引用坏声明、完整 selector、重复声明和首候选诊断优先级。独立模型的有限穷举与故障变体是补充证据，实际 compiler 用例拥有行为验收。

Level 1 复用局部格式、静态规则、变更模块测试；Level 2 复用 PR 影响路由和契约门禁；Level 3 保留完整 CI、coverage 与发布验收，不通过删除业务场景加速。`repo-hygiene` 对测试文本只清洗一次并复用 structuralLines；完整 findings 及顺序由旧新差分验证。

量化样本、精确版本、CPU/RSS、故障检测与线上收据由 [Root #2700](https://github.com/taichuy/1flowbase/issues/2700) 及 `tmp/test-governance/optimization-20261029/` 留档。基线为最新实际发布 v0.5.8，并另列相对本轮起始 47bdb3fb7 的增量。
