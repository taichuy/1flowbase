# 事件中心

[English](README.en.md)

事件中心位于 `/settings/events`。事件定义、工作流触发配置和通信指令绑定持久保存；事件实例、队列、生命周期信号和观察动态仅保存在当前宿主内存，重启允许丢失。消费者已有的业务记录仍由消费者管理。

## 三种触发时机

| 时机 | 事实 |
| --- | --- |
| 订阅时 `subscribed` | 一个新的有效逻辑订阅被激活；恢复配置、无变化保存和增加同订阅工作协程不伪造新订阅 |
| 诞生时 `published` | 有效事件被发布，分别尝试投递给当前原始消费者 |
| 消费时 `consumed` | 原始投递从队列取出、责任交给消费者；不等待成功或失败 |

独立订阅各有自己的队列。同一逻辑订阅的多个工作协程竞争领取。消费观察者只接收控制信号，它取走该信号不会再次为原事件产生消费信号。只有消费观察者而没有原始消费者时，发布事件不会制造消费。

`EventDelivery` 的资源额度保留到 guard 释放。取出计数、仍持有资源和业务成功是不同概念；观察页面通过快照读取，不能从队列取走消息。

## 使用路径

1. 在事件目录创建 `custom.<name>` 契约，设置版本和正文 schema。系统与插件注册来源只读；只允许选择已接入来源的触发时机。
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
| `observation_max_bytes` | 近期动态元数据的字节预算 |
| `observation_ttl_ms` | 动态保留窗口，毫秒 |

根据部署资源明确填写这些值。测试 fixture 中的小预算只用于制造边界，不是产品容量限制。当前视图包含宿主实例身份；不能把一个节点的值称为多节点总量。动态只保留元数据，不复制事件正文。

## 所有权与权限

后端 Settings 注册 `system.events`，每个 HTTP method/route 独立授权；workspace 隔离由服务及 repository 执行。目录读取不自动授予创建、修改或查看运行信息的权限。

事件服务复用宿主 `EventBus`；Workflow 复用现有发布与执行引擎；通信插件保留供应商 Receive/Send 职责。既有可靠 Outbox 和运行诊断流保持原契约，不因事件中心出现而转换为 ephemeral 消息。

新注册使用 `node scripts/node/tooling.js settings-registry register <descriptor.json>` 修改唯一 Core 注册源，并在验收批次检查 compiled inventory。描述符包含 `feature_id`、`console_surface` 与 `api_routes`，不建立第二套运行时权限映射。

两张新表只保存配置：`event_definitions` 的选择性备份归 `system.events`，`workflow_event_triggers` 归 `system.applications`。恢复时沿用现有依赖校验；选择应用配置不会暗中复制其他事件定义。内置模型的结构与 generic record 写入口受保护，专用服务负责修改。

## 验证入口

行为证据分为事件服务单元测试、`event_center_configuration` PostgreSQL target、`api_system_settings` 的 event center 路由用例、`api_private_contracts` 的 event workflow/communication 用例，以及对应前端 feature 测试和 page-debug 页面证据。有限设计模型不替代这些实现测试。

本机制不承诺历史回放、重启恢复、跨宿主广播或外部副作用 exactly-once。需要这些能力时重新确定范围，不能用观察日志暗中承担恢复责任。
