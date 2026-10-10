# 插件组合与事件交付

[English](plugin-composition.en.md)

本文维护当前组合契约。当前验收以实际提交与测试报告为准；[#2007 历史测试入口](archive/2007/plugin-composition-test-batch.md)不作为当前通过证据。通用命名空间事件的后续边界见[调用生命周期](interface-lifecycle.md#契约驱动的插件事件)，下述 Create → A → B/C 是具体实例。

## 治理、安装与授权

HostExtension 保留可信启动 / 重启边界；受管插件沿用 worker 进程。贡献、执行方式、作用域分别建模，旧 manifest / catalog 分类仅作为明确的输入格式归一化，不能继续用整包分类限制多贡献，也不批量改写历史配置。Rust native 插件不支持反复热卸载。

正式包经安装、工作区分配、贡献授权、候选图编译和执行绑定后才发布。manifest 中声明权限不等于获得权限。同包贡献不合并权限，安装身份、工作区、贡献、资源范围与契约版本共同限制调用。升级、新安装和旧权限迁移不自动扩大权限；worker 声明、因果标识、旧 claim 或已入队事件都不是新的授权凭据。

Extension Center 的 installed/:installation_id 下提供以下独立操作；POST 仍需有效 session、CSRF 与对应操作授权，旧 configure 权限不能替代它们。

| 相对路径 | 方法 | operation |
| --- | --- | --- |
| contribution-authorizations | GET | extension_center.contribution_authorizations.view |
| contribution-authorizations | POST | extension_center.contribution_authorizations.grant |
| contribution-authorizations/revoke | POST | extension_center.contribution_authorizations.revoke |
| managed-execution | GET | extension_center.managed_execution.view |
| lifecycle-deliveries/resume | POST | extension_center.lifecycle_deliveries.resume |
| managed-executions/retire | POST | extension_center.managed_executions.retire |

授权 DTO 示例：

```json
{"contribution_id":"acme.composition-a.events","permission":"event.subscribe","resource_scope":{"kind":"workspace"},"permission_contract_id":"managed-event","permission_contract_version":"1"}
```

撤销使用 `authorization_id` 和 `expected_revision`。A 的 `event.publish` 必须另行授权；B/C 的事件贡献还需 `plugin_data.owned.write`、`plugin-data@1` 与 `{"kind":"owned_collection","collection_code":"processed_models"}`。只给 `.data` 贡献授权不能让 `.events` 贡献写数据。

## 接口阶段与冻结执行

首批真实入口是 HTTP `POST /api/console/settings/data-models/model-definitions` 对应的 `model_definitions.create`。HTTP / MCP 进入同一 typed Kernel。开放 Authorization、Admission、Before、After、Failure、Completion 六阶段；认证适配器仍由可信宿主提供。核心拒绝不能被扩展 allow 恢复，Before 只读且可否决，观察阶段不能改写主结果。

每次调用冻结图、绑定、artifact、generation 和执行身份。候选只有在权限与版本仍有效时才能发布；过期候选不能恢复已撤销权限。旧调用继续持有原快照，新调用使用完整发布的新快照，禁止拼接不同版本或查找 latest 作为替代。

旧 Create 的 Before 否决是该 adapter 的特例；通用受管 Before 仅观察。阶段权限统一见[生命周期权限表](plugin-lifecycle-contracts.md#受管接口阶段权限)，不能从本节示例扩大插件权限。

## A → B/C 与两种事务

A 的原 manifest 在同一安装下绑定两个 Create.before 贡献和一个声明式 owned collection。事件变体订阅 `model_definition.committed@v1`，其 `publish` handler 发布 `acme.composition-a.processed@1`。B/C 分别以自己的 `apply_processed` handler 消费并写入各自 `processed_models`。安装验收先由真实 managed schema owner 应用表结构，再启用 B/C；插件作者不获得任意 SQL 入口。

Create 业务事实与其 Outbox 目标在业务事务中提交。A 派生 processed 事件使用独立 E2 事务；B/C 的 owned-data 副作用与消费 receipt 在同一 PluginData 事务原子提交。稳定幂等身份包含 installation / workspace / contribution / event，不用 worker generation 替代。此保证不覆盖外部服务的 exactly-once。

载荷只包含受约束的 `model_id`、`status`、`result_reference`；Host 绑定 causation / correlation 与可信身份，worker 不能自行指定。发布与重试仍检查当前贡献授权。进程内 AfterCommit lane 只提供临时幂等与等待，不替代持久 Outbox。

## 暂停、恢复与退出

旧目标保留精确 epoch、graph fingerprint、handler id/version、artifact 与绑定。重启后无法重建的旧 epoch 保守暂停，不能自动投递到当前版本。撤销或停用保留已暂停的持久行；重新 grant / enable 不自动 resume。

恢复必须提供 `event_id`、`subscriber_id` 和 `expected:{graph_fingerprint,handler_id,handler_version}`，重新验证精确目标及权限。退休同样指定完整目标；当前目标不可退休，目标引用与 backlog 均清空才可退出。所有被移除快照的绑定都记录精确退休标记。未知 legacy 历史保留并阻止 resume / retire / delete，不能通过删除证据解锁。

claim 身份每次领取独立，过期 worker ACK 不能确认新 claim。冻结引用、执行 owner、子进程与持久 backlog 各自有真实 owner，取消等待者不等于取消已接纳的业务工作。

## 有界资源与关闭

| 资源 | 当前上限 / 策略 |
| --- | --- |
| store / composition 托管操作 | 各 128，先取得 owner permit 再 spawn |
| 受管 mount / 调用 | mount 4096；每 scope 32、全局 128 |
| hash / loading | 4，permit 随真实阻塞工作保留 |
| 当前工作区 / 保留快照 / 冻结引用 | 不以固定业务数量拒绝；保留真实在途引用和待处理历史 |
| 退休标记 | 保留至宿主关闭；防止共享 worker generation 重建已退休目标，不按数量驱逐 |
| 治理状态历史 | 默认每页 256 条候选元数据，通过 `next_cursor` 续读；不读取历史 payload，不限制历史总量 |
| 退休 / 删除积压检查 | 安装 / 历史作用域 / 贡献限定的未完成元数据，最多检查 4096 条；应用与数据库各 5 秒预算，无法证明为空则返回 Busy |
| 受管 capability 消息 | request / stdout / stderr 各 1 MiB |
| 临时 AfterCommit lane | 4096，含完成幂等标记；容量失败 attempts=0 |
| Outbox dispatcher | 单批 32；deadline 10 秒且不越过 30 秒 lease；最多 5 次；诊断 4096 字节 |

状态响应保留 `deliveries`、`deliveries_truncated`，增加 `next_cursor`。GET `managed-execution` 将上一页游标作为 `cursor` 原样传回；未提供时读取第一页，末页返回空游标。游标绑定安装和工作区，不授予权限；格式错误或作用域不匹配明确拒绝。resume / retire 的响应仍返回第一页。

查询使用 `(event_id, subscriber_id)` keyset。游标来自最后已扫描候选，而不是最后返回项；过滤属于其他安装的历史后，页面可能为空，但有 `next_cursor` 就应继续。静态历史可完整遍历，不重不漏；跨请求不承诺数据库快照，期间状态变化由重新读取反映。单条 resume 保持按 installation / workspace / event / subscriber / graph / handler / version 精确读取，不依赖展示分页。

资源治理不能把内部集合长度变成未经业务定义的工作区、历史或调用容量。冻结引用使用既有引用计数和关闭通知；retained 快照只有经过显式退休、引用与持久积压检查后才释放。退休标记是防复活状态，不是可任意淘汰的缓存；同一 generation 可能被其他快照共享，因此保留到宿主关闭。这里不承诺内存恒定，也不根据开发机猜测部署容量。其他执行通道的背压、协议帧保护与关闭预算各有独立语义，不由这些业务数量推导。

退休 / 删除的安全检查不使用展示页：SQL 先限定安装、历史授权作用域、贡献与未完成状态，再有界遍历元数据。超过 4096 条或 5 秒预算返回 `managed_backlog_check_busy`（HTTP 409），不误判为空；已完成历史不参与检查。删除直接关联历史 scope，不先收集所有工作区。native subscriber 的精确目标使用 SQL EXISTS，在数据库内投影 Create / processed 的 workspace；未知 legacy 或不能验证的 scope 保守阻塞，不能通过分页绕过。

`ApiRuntimeShutdown` 先关闭 dispatcher 新批次并等待真实 batch（15 秒），再依次等待 composition owner（15 秒）、store Create/E2 owner（15 秒）、冻结引用（15 秒）。已有 Create 必须仍能冻结，故引用 gate 不能提前关闭。最后 Host 关闭运行时并等待 hash（5 秒）、scope / worker（5 秒），再做普通清理；成功后才清空 composition。超时报告未完成，不删除数据库 backlog。子进程 owner 在取消时仍持有 lease 直到 kill / reap；reap 失败保留 permit 并报告未完成。

required 事件通道保留背压，诊断通道即使接收者丢弃也保留计数。容量错误属于执行失败，不冒充契约失效并永久暂停目标。

## 同版本重装与历史投递

受管插件的同一 `plugin_id + version` 以安装时持久保存的原归档 SHA256 为内容身份。只允许重传**同一原 archive bytes** 恢复制品；安装目录、保存的归档缺失时也可恢复，保留 installation ID、Disabled 意图、贡献 metadata、授权修订与历史积压。相同内容不会仅因存在积压被拒绝。重新打包即使 manifest 看起来相同，只要归档 digest 不同就不是原制品，同版本重装明确拒绝；同步修改 descriptor 与 execution binding 或改为旧包类别也不能覆盖原身份。安装准入和提交使用同一数据库连接的身份锁，支持单连接池，并发首次安装不能让两个不同归档取得同一版本身份。

缺少可靠历史 checksum 的旧安装不能凭当前 manifest 认领或回填内容身份，恢复请求明确拒绝。新版本仍按独立安装、候选显式授权及正式切换执行；原版本授权不会自动授给候选，旧冻结投递也不转交新版本。

历史投递归属依据已持久的安装身份、workspace 与冻结目标，不依赖当前 manifest 的 contribution 列表。停用及完整 host/composition 重启后，旧暂停记录仍可查、精确定位，并阻止删除所需制品；原 epoch 或执行图不可用时明确拒绝恢复。未知 legacy 记录保守保留并阻止清理，不能由新 manifest 推断归属。
