# 通知实例与插件发送

通知有四个独立对象：类型描述、工作区通知实例、用户站内消息、发送调用日志。设置页保存实例配置；工作流通知节点选择实例并提供本次输入。底座只内置 `system` 类别，其他类别由受管插件贡献。

## 数据与结果

`notification_instances` 保存 `workspace_id`、`type_code`、`installation_id`、`display_name`、`status`、公开 `config_json` 和加密秘密配置。密码控件字段由宿主加密，查询只返回 `configured_secret_fields` 字段名。更新时省略密码字段会保留已有值。

`system_notifications` 按收件用户保存独立消息及 `read_at`。发送必须指定当前工作区有效成员，所有收件人验证成功后事务提交，随后发布刷新信号。消息列表、未读数和标已读都以当前工作区、当前用户过滤。删除实例、关闭或卸载插件保留已有消息；实例不存在、停用或插件不可用时发送明确失败。

站内通知成功表示消息事务已提交；外部通知结果由插件对供应商的接收确认定义。成功不表示用户已读。`notification.changed` SSE 只通知客户端回读 HTTP 快照；断线重连也回读数据库。刷新信号不是持久消息，不承诺客户端收到每个事件。

## 插件描述

完整示例见 [notification fixture](../../api/plugins/fixtures/acme.notification/manifest.yaml)。`notification_types` 声明以下字段：

| 字段 | 职责 |
| --- | --- |
| `type_code` | `publisher_namespace/plugin_id/local_type`，`system` 保留给宿主 |
| `config_schema` / `config_form` | 保存到实例的 JSON Schema 与既有 `PluginFormSchema`（`schema_version: 1.0.0`） |
| `input_schema` / `input_form` | 单次发送的参数与表单，与配置分离 |
| `output_schema` | 单次结果对象的 JSON Schema |
| `contribution_id` | 对应受管发送贡献 |
| `log` | 可选的标题、完整正文和附属信息映射，只读取业务输入与业务结果 |

表单使用已有 string、number、boolean、select 控件、默认值与条件；`control: password` 只用于配置中的 string 字段，`control: multi-select` 对应 array schema，选项值按 `items` 校验。Schema 自包含，不解析外部 `$ref`。系统类型的收件人选项由类型 API 按当前工作区成员提供。

贡献绑定到 `1flowbase.notification.send`、契约 `1flowbase.notification/v1`，请求 `notification.send` 权限，并声明 `payload: {kind: notification_type, type_code: ...}`。安装、工作区分配、贡献授权和启用沿现有插件生命周期；manifest 声明不等于授权。贡献授权使用 `permission_contract_id: notification`、`permission_contract_version: 1`、workspace resource scope。

## 实现发送

工作进程沿现有 capability envelope 接收 `method: execute` 与 `input.config_payload`、`input.input_payload`。共享 Host 负责实例校验、可信 actor/workspace、有效贡献快照、独立授权、进程分发与生命周期。插件负责供应商发送和输出，不维护第二套宿主通信或数据库连接。

Rust 作者可以复用 `runtime_extension_sdk::execute_notification`；stdio one-shot 示例见 [SDK worker](../../api/crates/runtime-extension-sdk/src/_tests/notification_worker.rs)。multiplex 插件在已有 `serve` handler 中反序列化 `CapabilityStdioRequest`，调用同一 helper 后序列化响应即可。首版支持 `process_per_call` 的 stdio 与 multiplex，不声明 notification 的 actor service pool。

```bash
cargo build --locked --manifest-path api/Cargo.toml -p runtime-extension-sdk --example notification_worker
export NOTIFICATION_WORKER_FIXTURE="$(pwd)/api/target/debug/examples/notification_worker"
node scripts/node/plugin/cli.js package api/plugins/fixtures/acme.notification \
  --out tmp/notification-packages --runtime-binary "$NOTIFICATION_WORKER_FIXTURE" \
  --target x86_64-unknown-linux-gnu
```

该 fixture 不联系外部服务。实际构建输出位置应沿本机 Cargo target 配置解析；上面的默认路径不是已执行证明。主仓不修改或发布 SSH/OpenAI 插件。

## 工作流和 API

节点 `type: notification` 保存 `config.instance_id`。`bindings.input` 使用既有 `named_bindings`，每字段沿描述中的 key，支持 constant、selector 和 templated_text；数字表达式沿 `valueType: number`。例如：

```json
{"config":{"instance_id":"00000000-0000-0000-0000-000000000000"},"bindings":{"input":{"kind":"named_bindings","value":[{"name":"text","valueType":"string","value":{"kind":"templated_text","value":"任务 {{ upstream.result }} 已完成"}}]}}}
```

发送输出为 `{instance_id, type_code, result}`。默认 `result` 输出变量使用选中类型的 `output_schema`；下游可选择 `result` 内的字段。已发布/恢复执行使用冻结的工作流 actor/workspace，不重新套用用户后来切换的工作区。调试单节点只发送本节点，不重放上游。

HTTP owner 是 `/api/console/notifications`：types、instances CRUD、`instances/{id}/send`、messages、unread-count、read、read-all 和 events。设置操作沿独立 console operations；本人消息与类型读取使用认证入口。业务 HTTP 经 Canonical Interface/typed Handler；events 是只读刷新协议控制入口。API、工作流复用同一个 `NotificationService`。

测试以实际 package intake、PostgreSQL、独立授权与 SDK 二进制覆盖发送、schema 拒绝、秘密脱敏和不可用实例保留；测试路径与执行证据记录在当前任务的 `tmp/test-governance/`，源码或复现命令本身不代表测试已通过。

## 发送日志与个人渠道

每次授权工作区发送先持久化一个 `notification_send_logs` pending，再执行校验和发送；HTTP 与工作流通过 `NotificationSenderPort` 显式指定可信 `api` / `workflow` 来源，客户端 body 不接受来源字段。日志保留实例 UUID，无实例/用户外键，也不回填历史。实例缺失时名称、类型为空。日志 list/get 是独立设置 operation，按工作区授权与查询。

站内消息与日志 success 在同一事务提交，保留去重后的 recipient_count。声明 `log` 的类型在供应商调用前保存经过校验和秘密检查的 `title`、完整 `content` 与输入附属信息；调用完成后仅合并显式声明的业务回执到 `extra`（JSONB 对象）。不保存完整原始 input/result、config、受管凭据、内部协议或原始错误。发送前的缺失实例、停用、不可用类型/插件、输入/配置/收件人校验失败有固定 failure_code。插件错误可能已产生外部效果，记录 unknown/provider_result_unknown；成功返回后的输出 schema/秘密校验失败记录 unknown/provider_output_invalid。日志收尾写失败保留 pending，不能把不确定外部结果写成确定失败。

`channels` 只聚合当前工作区当前用户的站内消息。`read_status=all|unread|read` 先过滤，再选渠道最新消息并按最新时间排序；每渠道 unread_count 独立计算全部未读事实。cursor 分页上限 100。消息接口支持 instance_id/read_status，保留 unread_only 兼容参数；显式 read_status 优先。删除实例后 left join 保留渠道，deleted=true，display_name='Deleted notification channel'，type_code='system'（历史消息表只保存系统站内消息）。

`messages/{id}/unread` 清除本人消息 read_at；`channels/{instance_id}/read-all` 只标记本人该渠道消息，包含已删除渠道历史。两个写入口都发布同一 recipient/workspace SSE 刷新信号。旧全局 read-all 保留。

## 插件声明发送日志

```yaml
log:
  content: /text
  extra:
    accepted: {source: result, pointer: /accepted}
    delivery_id: {source: result, pointer: /delivery_id}
```

`title` 可选，`content` 必需，均是指向已声明字符串输入字段的 RFC 6901 JSON Pointer；支持 `~0`、`~1` 转义。未给标题路径、输入未给标题或标题为空白时，从正文取前 255 个 Unicode scalar characters，正文不截断，显式标题保留。正文缺失或类型错误在调用供应商前拒绝。`extra` 按目标 key 显式声明 `source: input|result` 与 `pointer`，可投影 JSON 值，缺失可选值省略；禁止根指针、未声明字段、writeOnly 字段和非法转义。来源不包含 config、managed secrets 或 runtime envelope。

宿主检查完整消息投影中的已知配置密码和受管凭据，敏感输入投影在供应商调用前拒绝，不落入正文或附属信息。结果先通过业务输出 schema 和现有秘密检查，再提取回执；调用失败或不确定结果保留已持久化消息。界面消费后端生成的标题和完整正文，以键值结构展示附属信息，不按供应商猜测字段。未声明 `log` 的旧类型仍可加载并保留原来的 metadata-only 记录；历史缺失正文不推测回填，`extra` 迁移默认为 `{}`。

## 通知实例扫码授权

类型描述的可选 `authorization` 声明扫码动作和 `managed_secret_keys`，缺省仍走原始发送契约。授权沿同一个 `1flowbase.notification.send` 贡献与 Capability Execute managed dispatcher，不注册模型 provider 或通知 slot。Rust 插件复用 SDK async `execute_authorizable_notification` 与共享 `serve`；授权 fixture 顶层和 execution binding 都使用 `stdio_json_multiplex_v1`，旧 one-shot fixture 保留 `stdio_json`。

`POST /api/console/notifications/instances/{instance_id}/authorization` 接收 canonical `NotificationAuthorizationOperation`，body 为 `{type:"begin",action:"qr"}`、`{type:"poll"}`、`{type:"cancel"}` 或 `{type:"maintain"}`。该写入口需要 CSRF 和独立 `notifications.instances.authorization` console operation，经 Canonical Interface/typed Handler 再由 service 检查工作区与权限。历史设置 grant 不默认获得这项新授权，root 保留既有管理员权限。

响应的 ApiSuccess data 和实例 DTO 的 `authorization` 使用同一个 `NotificationAuthorizationView`：`status=unauthorized|pending|authorized|invalid`。pending 必须含 `user_action={kind:"qr_code",qr_code_url,poll_interval_seconds,expires_at?}`，仅展示合理 HTTPS QR URL，不返回内部二维码 challenge、bot token、context token、绑定收件人或供应商错误。legacy 实例的 authorization 为 null。管理员 `status=enabled|disabled` 与账户授权完全独立，授权成功不会启用已停用实例。

宿主在 `notification_instances.authorization_json` 保存公开状态，在 `encrypted_managed_secret_json` 加密管理凭证，和用户可编辑密码配置 `encrypted_secret_json` 分开。两者都绑定 workspace/instance 身份，并使用独立 AAD；selective backup 的现有 rekey 路径同时处理两类密文，跨 master key 恢复时不丢失授权。管理字段不能放进公开 config 或由客户端设置，patch 只接受描述符声明键，null 删除，省略保留。插件可以加密缓存可展示 `qr_code_url`；只有授权 `user_action.qr_code_url` 的同值显示字段可公开，发送 result、expiry 或其他字段中的该管理值仍会被秘密反射校验拒绝。

`authorization_revision` 是持久化 CAS fence。begin/cancel 在网络调用前占新 revision、清除旧管理凭证；插件调用期间不持数据库事务锁。poll 返回后只在 revision 未改变时原子写公开状态和加密秘密，迟到 poll 无法覆盖 cancel/re-begin。发送返回的旧 patch 也不能替换新授权；外部发送确认不会因为旧 patch 被丢弃而改成失败。进程重启后从宿主存储恢复 pending challenge 或已授权凭证，不依赖 worker 内存。

授权通知的 Execute input_payload 是 `NotificationRuntimeInput`，含 `operation`（authorize 或 send）和 Host 注入的 `managed_secrets`；用户 config_payload 继续独立。runtime result 是 `NotificationRuntimeResult`，公开授权视图和管理秘密 patch 分离。宿主独立验证响应、allowlist、输出 schema 和秘密反射，不信任作者 SDK 已做校验。未经验证的 patch 不保存，raw result 不写日志。

发送前 unauthorized/pending 在 service 本地拒绝，日志为 failure/authorization_required；invalid 本地拒绝并记录 failure/authorization_invalid。authorized 允许直接尝试供应商发送，不要求 context token 或 recipient-ready 布尔前置。供应商明确返回 recipient_not_ready 时账户仍 authorized；明确 provider_rejected 记录 failure；请求错误、异常返回、秘密泄漏或无法确认外部结果均保留 unknown/provider_result_unknown，不虚报成功。供应商可在成功、拒绝或 unknown 响应中提交有效游标/context patch，宿主加密保存。真实供应商只匹配扫码绑定用户的入站 peer，不能用其他 sender 重绑定目标。

授权集成 fixture 入口在 API tests 的 `extension_bus/notifications/authorization.rs`；构建 SDK `notification_authorization_worker` 后设置绝对路径环境变量 `NOTIFICATION_AUTHORIZATION_WORKER_FIXTURE`。它覆盖真实安装、独立贡献授权、canonical HTTP、加密 PostgreSQL、重启恢复、受控迟到 poll、固定失败日志及 legacy system 回归；fixture 不表示真实微信送达。实际执行证据仍由 Root 的集中 QA 记录。
