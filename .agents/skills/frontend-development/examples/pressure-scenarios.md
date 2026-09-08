# Frontend Pressure Scenarios

## Use

修改 skill / AGENTS 或发现规则误伤时，挑选受影响场景检查最终决策。以下是行为期望与反方样例，不是产品测试通过记录；普通页面开发无需逐项执行。仅凭规则文本不能证明未来 agent 一定遵守。

## Decision Scenarios

| 请求 / 事实 | 应产生的决策 | 反方样例 / 不能推出的结论 |
| --- | --- | --- |
| “只调这个按钮的间距”，无行为变化 | 给局部可见效果，按 diff 选择必要验证 | 不要求全量 build、全部页面或重新需求对齐 |
| 一个 feature 新增请求，无跨 feature 编排复用 | `api-client -> feature api -> UI` | 不为凑三层创建 `shared/api`；多个 feature 真实共享编排时可以提取 |
| 一个页面有 30 行 JSX，只有一处使用 | 按状态归属和变化原因判断是否局部拆分 | 行数和“以后复用”不证明需要公共组件；独立职责仍可拆分 |
| 页面根组件维护请求、表单、选择和多个弹窗 | 按各自 owner 收敛状态与消费逻辑 | 不把服务端真值复制进全局 store，也不一律拆成微型 hook |
| “在 home 顺便放完整运行日志” | 核对任务域；完整日志按既定应用 section recipe | 内容更多不证明首页边界合理；新产品方向交 problem-framing |
| 已确认 Shell 行详情使用 Drawer | 复用交互规则直接实现 | 不重新要求一份需求草案；新第三种 L1 模型属于另一个决策 |
| “页面不顺手”，入口与对象行为冲突 | 用交互证据卡给 problem-framing 提供结构证据 | 不先改 spacing，也不在前端 skill 内另开审批流程 |
| 只有参考图，页面目标与核心动作未明确 | 提取结构 / 信息层级线索并进入 problem-framing | 不把图里的按钮当授权；用户明确直接实现时按授权范围判断 |
| 移动端桌面画布必须横向滚动才可操作 | 使用已确认的摘要与引导降级 | 压小字体不证明小屏可用；纯 DTO 修改不需重拍移动端 |

## Contract Scenarios

| 请求 / 事实 | 应产生的决策 | 反方样例 / 不能推出的结论 |
| --- | --- | --- |
| 编辑角色：关闭 group 后保存，再重开 | 保留 strategy 与自定义 operations，按后端 contract 往返 | `enabled=false` 不等于清空配置；用户明确修改某 operation 才改变对应草稿 |
| 同 feature 中只授权一个 operation | 消费单接口 operation policy 和 catalog | 页面可见不代表同 feature 所有 API 都已授权；UI 测试不能证明服务端 403 |
| API catalog 已含 summary / description | 按原字段展示 API owner 的说明 | 不从 URL 生成说明或给 operation 增设 label_ref；普通按钮仍用 UI i18n |
| SSE 断开且没有收到业务终态 | 记录连接状态，按既有接口恢复观察 / 查询 | EOF 不代表成功、取消或回滚；已收到有效终态则可以展示对应结果 |
| 取消接口接受请求，但业务仍 running | 区分取消请求与确认终态，继续按 contract 观察 | 接受取消不证明远端停止，也不自动重发可能已提交的 mutation |
| 切换对象后旧请求才返回 | 旧请求不覆盖新上下文，清理归订阅 owner | 前端 abort 不证明后台取消；独立且仍被使用的共享订阅不应一并关闭 |
| i18n hygiene 提示重复 value | 保留文案，按语义复用 key 或记录保留原因 | 不为了消除 warning 改文案；用户明确要求改文案仍可按范围修改 |

## Evidence And Review Boundary

- 历史冲突：2026-09-08 之前的需求 references 在主 Skill 已移交需求决策后，仍存在“整理后默认实现”链路；旧 API chain 把 `shared/api` 当必经层。可通过 git 历史核对，不保留失效执行模板。
- 新授权与状态场景依据 [consumer-contracts.md](../references/consumer-contracts.md) 的后端规则、DTO 和架构入口；验证层级依据 [review-checklist.md](../references/review-checklist.md)。
- 人工确认只落在改变产品目标、授权、业务终态、交互 contract 或成功标准的未决事项；已有确认和直接实现授权持续有效。
- 本次相关场景能导出一致决策、引用可达且没有冲突规则时结束检查；不为增加覆盖率扩成产品全量 QA。
