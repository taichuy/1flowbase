# 插件组合与事件交付

本文描述当前源码的有限机制。源码、示例与验收 fixture 已提供；Root #2007 的集中 QA 尚未执行，本文不构成验收通过或生产就绪证据。

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
| 当前工作区 / 保留快照 / 冻结引用 | 各 256；满额拒绝，不驱逐存活对象 |
| 退休标记 | 4096；满额拒绝 |
| 受管 capability 消息 | request / stdout / stderr 各 1 MiB |
| 临时 AfterCommit lane | 4096，含完成幂等标记；容量失败 attempts=0 |
| Outbox dispatcher | 单批 32；deadline 10 秒且不越过 30 秒 lease；最多 5 次；诊断 4096 字节 |

`ApiRuntimeShutdown` 先关闭 dispatcher 新批次并等待真实 batch（15 秒），再依次等待 composition owner（15 秒）、store Create/E2 owner（15 秒）、冻结引用（15 秒）。已有 Create 必须仍能冻结，故引用 gate 不能提前关闭。最后 Host 关闭运行时并等待 hash（5 秒）、scope / worker（5 秒），再做普通清理；成功后才清空 composition。超时报告未完成，不删除数据库 backlog。子进程 owner 在取消时仍持有 lease 直到 kill / reap；reap 失败保留 permit 并报告未完成。

required 事件通道保留背压，诊断通道即使接收者丢弃也保留计数。容量错误属于执行失败，不冒充契约失效并永久暂停目标。

## 官方 CLI 复现与证据

SDK 与 fixture 的构建、打包命令见 [SDK README](../../api/crates/runtime-extension-sdk/README.md) 和 [A fixture](../../api/plugins/fixtures/acme.composition-a/README.md)。使用仓库根目录、Linux、锁定依赖与实际 SDK worker，不能把旧 Python fixture 当成 typed Hook/event worker。

有限门禁 scope 为 `plugin-composition-2007`。Root 冻结并推送候选后，从同一候选 ref dispatch 工作流：

```bash
gh workflow run quality-gate.yml --ref "$CANDIDATE_SHA" \
  -f scope=plugin-composition-2007 -f target_branch="$CANDIDATE_SHA" \
  -f candidate_sha="$CANDIDATE_SHA"
```

本地等价入口要求两个显式数据库 URL，复用测试支持的隔离 schema / migrations，不直接使用生产库：

```bash
export PLUGIN_COMPOSITION_CANDIDATE_SHA="$(git rev-parse HEAD)"
export DATABASE_URL='postgres://postgres:1flowbase@localhost:5432/1flowbase'
export API_DATABASE_URL="$DATABASE_URL"
node scripts/node/plugin-composition-test-batch/runner.js
```

runner 校验 checkout / workflow SHA，串行构建真实 SDK examples 和 12 个 Rust test target，核对 47 个必需完整测试名，与既定回归过滤范围合并去重后逐项精确执行；另运行 4 组 Node 命令。缺名、零测试、忽略、失败、环境缺失都不能算通过。实际回归总数由编译产物 `--list` 决定，不预报通过数。报告及逐命令日志位于 `tmp/test-governance/2007`，工作流始终尝试上传 artifact。AC / AUTH 映射是证据索引，最终验收由 Root 集中 QA 结算。
