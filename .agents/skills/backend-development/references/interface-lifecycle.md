# Interface Lifecycle Implementation

## Read By Change

架构唯一说明入口：[请求架构与调用生命周期](../../../../docs/architecture/interface-lifecycle.md)。先读总体结构，再按改动定位相应章节。

| 改动 | 读取 |
| --- | --- |
| 新入口、mount、Catalog、MCP/WebMCP | [入口与owner](../../../../docs/architecture/interface-lifecycle.md#ingress-and-ownership) |
| 认证、Binding、Principal、执行变体 | [身份与冻结](../../../../docs/architecture/interface-lifecycle.md#contracts-and-identity) |
| Kernel、Decision、Hook、Handler | [执行与扩展](../../../../docs/architecture/interface-lifecycle.md#execution-and-extensions) |
| stream、取消、deadline、事务、交付 | [终态与交付](../../../../docs/architecture/interface-lifecycle.md#finalization-and-delivery) |

## Implementation Decisions

1. 先确定业务意图、现有协议契约、入口分类和owner。复用已有Definition/Binding时确认语义与typed contract一致，不按名称相似猜测。
2. 入口改动通过宿主受控assembly同时贡献mount和Inventory；将factory/handler绑定进冻结Registry，不另开旁路或维护独立路由清单。WebMCP外层Invocation不能由下游业务调用代替。
3. 认证改动保留attempt lineage与拒绝证据；同HTTP carrier变体只使用既有受限选择边界，不在业务Resolve后更换计划。
4. 业务修改留在typed Handler后的service/action；Kernel只执行已绑定ports。检查实际Cargo依赖，不能把逻辑调用图变成实现层依赖。
5. 流式改动画出event、terminal、completion各自owner与任务退出路径，覆盖socket close、writer失败、bridge abort；正常路径持有句柄不等于异常路径仍有收尾owner。
6. 交付给QA的是有限场景、旧契约依据、期望状态/副作用和未覆盖项，不能仅给编译通过结果。验证边界见[等价证据](../../../../docs/architecture/interface-lifecycle.md#equivalence-and-evidence)。

## Failure Examples And Evidence

- 给route补Catalog条目但没有实际mount：不能结算覆盖；使用真实assembly正反例。
- 对两端字段数组排序后比较：掩盖协议顺序差异；保留顺序并先验证本端返回ID对应落盘对象。
- 给断连路径直接加业务cancel以修“收尾丢失”：改变产品语义；修completion owner，显式取消另测。
- 移除Compact的原capability=false初态：失去旧行为基线；成功/失败原fixture与callback分支分别保留。

这些反例来自既有接口验收路径；按本次影响面选择必要测试，不要求每次后端改动跑全部协议矩阵。纯文档变更以源码/链接/语义核对替代TDD，Rust/PG/在线重型验证交CI。

## Stop

需要新增协议操作、改变权限/事务/取消语义、扩大插件开放或内部调度覆盖时回到problem-framing。已批准范围内的装配与fixture修正继续执行；不为收绿弱化断言或把未执行项标为通过。
