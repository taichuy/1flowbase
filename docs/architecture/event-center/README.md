# 事件中心

[English](README.en.md)

事件中心位于 `/settings/events`。用户接口事件规则、工作流触发配置和既有通信指令绑定持久保存；自定义事件实例、事件中心投递队列和观察动态仅保存在当前宿主内存，重启允许丢失。系统既有 runtime / lifecycle / communication 来源通过其真实 owner 接入；其原有持久化、重试、fence 与 ACK 保持不变，不能因观察日志临时保存就推断原始事件也不持久化。

三个页签使用独立 URL：`/settings/events?tab=catalog`（事件目录）、`/settings/events?tab=queues`（当前队列）、`/settings/events?tab=activity`（近期动态）。直接打开 `/settings/events` 默认进入目录；刷新、前进和后退保留 URL 对应页签。页面沿用应用管理的共享表格工具栏和列设置，不另设标题面板。

## 系统来源与真实队列

事件目录管理当前事件定义：同一作用域内的 `contract_id` 是事件标识，对应一份当前 Schema，不再通过契约版本区分事件。创建、订阅、通信事件绑定和事件中心信号均不携带版本。`source.selection` 保存稳定来源身份、明确 scope 和后端声明的子类型 `filters`，技术详情中的 `point_id` 表示底层扩展接入点。Schema 来自 typed owner 或受管声明；`null`、布尔 Schema 和对象均由共享 JSON 区块原样展示。已有运行流声明使用真实 `RuntimeEventEnvelope` Schema，不为每个动态 `event_type` 伪造字段规则。

已接入范围包括运行时 required / diagnostic / after-commit、通信命令、原生模型与积分通知，以及冻结图允许的 managed publications。内部 `Notify`、watch 和任意 socket 不因此变成可订阅的业务事件。可用触发时机由实际 adapter 决定。纯声明模板和 Unbound 历史来源不作为普通目录行重复展示，原始历史队列仍可在运行视图检查。目录按作用域、事件标识和子类型 filters 区分；子事件继承同一原始 source，不重复发布事实。`group_path` 由后端声明业务层次，GUI、Workflow 与 MCP 消费同一份目录。

“当前队列”分别展示原始来源队列和事件工作流队列。required / diagnostic 的统计来自原始通道；durable pending / paused / claimed 来自现有 outbox 的只读聚合。没有可靠累计真值的字段显示未知，不补零。读取快照不取走消息；consumed 在原始责任交接时成立，后续 handler 失败或 ACK 不撤销它。diagnostic 丢弃与 required / durable 交付规则均保留原语义。

运行视图按 scope 分开保留动态、费用与丢失窗口，不合并不同窗口为伪全局计数。Workspace 与 System 即使使用相同 UUID 也不是同一范围；普通用户只看到当前工作区，root 可看 System 和 Unbound。旧 outbox 没有可信 scope 的行保持 Unbound；新元数据只保存可信 scope、生产者与因果工作流 ID，不复制 canonical 正文。队列刷新失败明确提示，不把失败当作空队列。

工作流选择持久保存可选 `source` 身份；不设置 source 表示自定义事件。系统来源必须由 root 选择，工作区来源不能跨工作区。工作流祖先 ID 沿真实运行、持久发布和内部 MCP 调用传递；同一因果链中的工作流不再次进入，不使用任意固定深度阈值。近期观察不保存正文，只有真实匹配的目标才延迟投影源正文，运行记录仍仅保存映射输入和必要身份元数据。

来源定义、schema、队列身份和 activity 共用 `observation_max_bytes`。中心持有唯一队列观察台账；runtime 的 accepted/dequeued/dropped 只原位更新已接纳的单个队列，不复制或序列化第二份队列 map。接纳时预留数值最大文本宽度以及 map/handle 的固定元数据费用；这是一套明确计费口径，不是进程 RSS。队列退休立即释放身份字符串，原始 owner 仍持有的 handle 克隆继续计费，直到最后一次 drop。

`queue_observation_complete=false` 表示部分队列元数据未获接纳。即使可见队列为空，也要显示观察不完整；缺失计数不能当成零。该状态不改变触发能力或原始投递，直到权威 owner 完整刷新成功或来源 owner 清理后才恢复。可靠来源继续使用原有绝对快照全量刷新。

## 目录与定义归属

目录默认显示名称、事件标识、来源分类、作用域、启用、可用触发时机和操作。所属模块与发布点放入只读详情；不再展示契约版本、注册状态或观察覆盖列。系统/插件的所属模块来自定义方，自定义事件没有所属模块，工作区归属由 scope 表达。来源分类根据定义方确定，不根据消费者是否通过插件执行决定。

后台 DTO 对自定义事件提供布尔 enabled；系统/插件为 null，不能通过自定义事件写入口启停。目录与详情统一使用开关展示：自定义事件按 enabled 显示，目录在有更新权限时可直接启停；系统/插件显示为开启且禁用，不把展示状态写回 enabled。来源接口读取失败、队列观察预算不足仍明确报告，不能因为删除诊断列而掩盖异常。自定义事件正文按当前 Schema 校验；只保存配置，不增加事件正文历史或旧工作流迁移机制。

原始插件、native lifecycle 和 outbox 的协议版本属于来源适配器的既有协议身份，仍由原始派发器校验；它们不进入事件中心定义、来源ID或订阅匹配，不存在隐藏默认事件版本或另建版本映射表。

## 三种触发时机

| 时机 | 事实 |
| --- | --- |
| 订阅时 `subscribed` | 一个新的有效逻辑订阅被激活；恢复配置、无变化保存和增加同订阅工作协程不伪造新订阅 |
| 诞生时 `published` | 有效事件被发布，分别尝试投递给当前原始消费者 |
| 消费时 `consumed` | 原始投递从队列取出、责任交给消费者；不等待成功或失败 |

独立订阅各有自己的队列。同一逻辑订阅的多个工作协程竞争领取。消费观察者只接收控制信号，它取走该信号不会再次为原事件产生消费信号。只有消费观察者而没有原始消费者时，发布事件不会制造消费。

`EventDelivery` 的资源额度保留到 guard 释放。取出计数、仍持有资源和业务成功是不同概念；观察页面通过快照读取，不能从队列取走消息。

## 业务事件与接口生命周期

- **接口规则**：`before` 位于认证、授权和准入之后、业务执行之前，携带明确开放的真实 `input`。`completion` 等到逻辑调用结束，`terminal` 为 `completed / failed / rejected / cancelled`；空 `results` 选择全部终态。流式等待逻辑流收尾，连接关闭不自动等于取消。成功才携带声明的真实 `output`，其他终态不补造输出；没有可信身份/工作区的认证拒绝不派发工作区规则。观察不能修改请求、准入或主结果。未声明字段、credential、native handle 和原始协议内容不会自动开放。通知发送的 `input_payload` 为业务 JSON 对象，不从样例推断供应商字段 Schema。
- **应用运行**：`host.application.run.started.v1`、`host.application.run.ended.v1` 复用现有状态 owner。首次进入 running 产生开始；最终事务/CAS 赢者提交后产生结束，覆盖 succeeded、incomplete、failed、cancelled。`run_mode` 区分 API、工作流 HTTP、定时、事件和调试等来源。LLM 节点回调、等待和恢复属于同次运行，不重复宣布开始/结束；观察没有第二套运行状态机。
- **Agent Logs**：JSON、OTLP 接收和控制台导入投影 typed 输入及真实 receipt，使用 `host.application.operation.<interface_id>.<before|completion>` 记录操作及可信 application/workspace 身份，无需伪造 FlowRun 或先上报 task_end。接收/导入完成与外部任务实际完成独立。失败不把 partial receipt 伪装为成功输出；原错误保留已提交数量。
- **积分**：`credit.integration_notification` 展开 CreditReserved、CreditSettled、CreditReleased、CreditGranted、CreditCharged、CreditRefunded、CreditAdjusted、CreditChargeSettingChanged、CreditCommandRejected 九个叶子。通过 `/payload/event_type` 匹配同一原事实，保留发布和消费身份。
- **通信**：`system.communication.received` 在新增消息去重并提交后产生，重复输入不重发。Text/File 保留真实 MIME 和文件描述，不开放下载凭证。`system.communication.send_result` 提供 accepted、failed、unknown 叶子，保留回复与原供应商消息的关联。accepted 只表示供应商接受，不代表送达/已读；中断恢复的 unknown 不触发盲目重发。微信当前出站仅文本，入站文件用 MIME 区分图片、音频、视频等。

```text
接口调用 ── before(input) ── 逻辑终态 ── completion(output 或 terminal/classification)
业务 owner ── 提交/真实回执 ── 业务事件 ── 各订阅独立队列 ── 消费者竞争取出
应用 run ── 首次 running ── 等待/回调/恢复 ── 最终状态提交 ── ended
```

接口完成、应用结束、订阅激活、消息消费和通知送达各有独立含义。跨订阅广播是扇出关系，不新增“广播”生命周期状态。

旧自由事件定义保持可读及原发布契约；缺少 `interface_event` 的旧记录在编辑器中只读，不借旧入口新建自由 Schema。通信指令的原发布契约只适用于旧自由事件，不允许伪造接口规则的调用事实。增量迁移只增加 nullable 规则元数据，不删除其他环境配置。当前开发库盘点未发现旧定义、触发器或通信事件绑定引用，这不代表所有环境均为空。

## 使用路径

1. 在事件目录新建接口事件规则：选择已声明的 Canonical Interface 操作，再选“请求前”或“响应结束后”；响应结束可筛选全部或指定终态。填写名称，查看继承的只读 Schema。事件标识由后端生成，不再自由填写 `custom.*` 或 Schema。用户规则在有更新权限时可启停；系统与插件统一显示开启且禁用的开关。
2. 在应用中创建 Workflow，选择 Event 触发类型，保存事件、阶段与输入映射，再发布工作流。未发布、无权限或来源不可用时不激活订阅，当前队列页显示原因。
3. 输入映射使用 RFC6901 指针，例如 `/payload/order_id`；沿用现有 Start 的 string/number/boolean 输入规则。只把明确映射的输入和必要事件标识写入工作流运行，不额外保存完整事件正文。
4. 通信绑定可新增 `user/<name>` 指令，并映射 `/arguments`、`/input_id`、`/conversation_id`、`/instance_id` 到事件字段。底座、插件与用户指令不能重名；不覆盖 `/new`、`/model` 或 `/help`。
5. 发送通知仍使用工作流已有通知动作。指令的接受回复不表示工作流已完成或通知已送达。

发布结果逐订阅报告准入情况。零目标合法但不留待后续订阅；部分拒绝不回滚其他目标，不自动向所有目标重发。消费发生与其观察工作流是否准入分别记录，后者失败不能撤销已经发生的交接。未知的先前指令结果不盲目再次发布。

## 宿主资源配置

`API_EVENT_CENTER_POLICY` 是 JSON 格式的显式宿主策略，没有猜测生产环境容量的默认值。缺少策略时，目录仍可管理，事件运行不可用，页面显示未配置。设置后需重启宿主。

| 字段 | 含义 |
| --- | --- |
| `bus_limits.max_items` | 预留、排队和持有中投递共同占用的条数 |
| `bus_limits.max_json_bytes` | 共同占用的序列化 JSON 字节上界，不等同于进程 RSS |
| `bus_limits.max_waiters` | 等待者预算 |
| `bus_limits.max_topics` / `max_consumers` | 活动队列与消费 owner 预算 |
| `bus_limits.max_topic_name_bytes` | 队列目录名称的字节预算 |
| `observation_max_bytes` | 近期动态与 workspace 计数账本元数据共用的字节预算 |
| `observation_ttl_ms` | 动态保留窗口，毫秒 |

根据部署资源明确填写这些值。测试 fixture 中的小预算只用于制造边界，不是产品容量限制。当前视图包含宿主实例身份；不能把一个节点的值称为多节点总量。动态只保留元数据，不复制事件正文。`observation_bytes` 是本 workspace 当前可见动态的费用（序列化 JSON 加条目元数据）与可用计数账本的费用之和；共享宿主预算也约束计数元数据，这些费用不等同于进程 RSS。

`observation_loss` 按 workspace 提供 `expired`（动态达到 TTL）、`evicted`（未到 TTL 因预算被移出）与 `oversize_dropped`（新动态连同其自身计数账本无法放入预算）数量。计数对象是观察记录，不是去重事件数或业务投递丢失数；达到无符号 64 位上限后饱和。

计数覆盖从 `window_started_at_unix_ms` 开始的有限窗口，`window_duration_ms` 为动态 TTL 的两倍（毫秒值在无符号 64 位上限饱和）。多出的一个 TTL 让没有新动态的 workspace 仍能在动态刚过期时看到过期数。快照只读投影过期，不修改存储或取走投递；后续写入把同一过期落实一次，不重复计数。动态可以早于当前计数窗口，只要丢失发生在当前窗口内就纳入计数。

计数窗口结束后，直到下一条记录开启新窗口前统计不可用；预算压力也可能移出 workspace 的计数账本。两种情况下 `accounting_available` 为 false，窗口开始值为 null，零计数表示覆盖缺失，不能解释为没有丢失。新窗口不继承旧计数。TTL 为零或预算小于一个计数账本时无法提供统计覆盖。后续写入回收过期存储，没有新写入时保留存储也仍有界。计数账本不是无界的 workspace 生命周期映射，其他 workspace 的丢失数不会计入当前调用者；重启允许动态与计数一起丢失。

## 所有权与权限

后端 Settings 注册 `system.events`，每个 HTTP method/route 独立授权；workspace 隔离由服务及 repository 执行。目录读取不自动授予创建、修改或查看运行信息的权限。

事件服务复用宿主 `EventBus`；Workflow 复用现有发布与执行引擎；通信插件保留供应商 Receive/Send 职责。既有可靠 Outbox 和运行诊断流保持原契约，不因事件中心出现而转换为 ephemeral 消息。

新注册使用 `node scripts/node/tooling.js settings-registry register <descriptor.json>` 修改唯一 Core 注册源，并在验收批次检查 compiled inventory。描述符包含 `feature_id`、`console_surface` 与 `api_routes`，不建立第二套运行时权限映射。

两张新表只保存配置：`event_definitions` 的选择性备份归 `system.events`，`workflow_event_triggers` 归 `system.applications`。恢复时沿用现有依赖校验；选择应用配置不会暗中复制其他事件定义。内置模型的结构与 generic record 写入口受保护，专用服务负责修改。

## 验证入口

本轮业务事件的集中验证入口为 `event-lifecycle.yml` 的 `event-business` scope，保存精确 source SHA、非零测试结果和匹配的宿主候选产物。接口观察、应用终态、积分筛选、通信去重/结果与权限分别验证；源码检查不代替运行结果。

行为证据分为事件服务单元测试、`event_center_configuration` PostgreSQL target、`api_system_settings` 的 event center 路由用例、`api_private_contracts` 的 event workflow/communication 用例，以及对应前端 feature 测试和 page-debug 页面证据。有限设计模型不替代这些实现测试。

本机制不承诺历史回放、重启恢复、跨宿主广播或外部副作用 exactly-once。需要这些能力时重新确定范围，不能用观察日志暗中承担恢复责任。
