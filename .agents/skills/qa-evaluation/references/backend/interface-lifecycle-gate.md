# Interface Lifecycle Acceptance

## Scope And Reading

适用于入口装配、认证关联、Kernel阶段、流式收尾及接口重构等价验收。先读[架构总览](../../../../../docs/architecture/interface-lifecycle.md)和[等价证据](../../../../../docs/architecture/interface-lifecycle.md#equivalence-and-evidence)，再按风险读取其余章节；不默认要求无关后端改动执行整套矩阵。

## Finite Acceptance Matrix

| 风险 | 必须取得的直接证据 |
| --- | --- |
| mount/分类 | 实际Router与Inventory同源，未知/重复/缺Binding/无mount拒绝发布；真实MCP/WebMCP外层调用 |
| 认证与冻结 | 原凭证不向下传播；成功/拒绝关联identity；Principal未建立不伪造；同carrier变体正例及跨snapshot/认证/contract负例 |
| 执行计划 | core deny不可恢复；空计划合法；已绑定计划按序执行；缺实现或不合法绑定拒绝；旧新快照互不混用 |
| unary/stream终态 | success/failure/rejection/cancel/deadline的适用路径；observer失败/panic/超时保留主结果和真实执行状态 |
| stream owner | 真实Kernel+生产bridge覆盖abort、writer关闭、缺terminal和正常收尾；不以复制实现的计数器代替 |
| 协议等价 | 基线输入接受/拒绝、DTO字段与数组顺序、完整status/error/message、流式事件；各端响应ID对应本端持久ID后才做规范化 |
| 业务效果 | 真实service/PG commit/rollback、完整grant/审计及非目标行不变；提交、调用终态、delivery/ack分别核对 |
| 在线并发/取消 | 受控barrier证明动作先于业务终态；服务端连接重叠、run/nonce隔离；断连与Native显式取消分别验证 |
| 上游错误 | 每个协议×错误行实际执行；原始错误、wire terminal、Native/console状态对应；retry失败与后续成功分别记录 |

只选命中风险且已批准的行；未知预期先标未验证，不临时创造业务契约。协议包装可以不同，但必须分别符合基线；不得排序业务数组、删错误字段或接受任意终态来制造等价。

## Evidence Settlement

- 记录精确执行SHA、配对provider版本、命令/场景和实际执行数；过滤器总计零测试不能算通过，正常空doc/bin子目标不等于整条命令零测试。
- 声明的matrix数量、直连mock请求和真实Gateway请求分开计数；重复Cargo filter不累加为唯一测试数。
- 失败行保留已取得的event、时间、trace、run/nonce和durable证据；取证失败单列，不能只保存成功行。
- 证据复用保留原SHA，说明相关源码/fixture/config身份与影响面；旧本地快照的pending字段不能冒充最终状态。
- 同provider池既有串行策略不能当作并发回归；连接并发通过不证明吞吐/延迟等价。业务cancelled不证明远端provider物理计算已终止。
- 分别结算架构边界、协议/效果、测试前置与全仓门禁；Seed启动失败、schema分类、旧PGfixture等先归因，不能由全局红灯推出重构回归，也不能由专项绿灯推出全仓通过。

## Resource And Stop

Dev Acceptance复用有效证据，只补影响面内缺口；Rust/PG/在线重型验证交同一冻结候选CI，不在本地并行起Cargo或业务服务。纯文档/skill更新核对链接、规则语义、反例和引用可达性，不启动全套后端门禁。

不能建立确定性时序、取得实际执行结果或确认基线时，相关项标未验证。需要改变协议、权限、取消或事务语义时回到Root定界，QA不自动修业务；同根因重复失败按既有批次停止规则处理。
