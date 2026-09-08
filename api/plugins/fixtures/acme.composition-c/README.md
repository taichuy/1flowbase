# 组合插件 C

本 fixture 使用 `apply_processed` handler 订阅 A 的 `acme.composition-a.processed@1`，在自己的 `processed_models` 中写入 `model_id`、`status`、`result_reference`。`.data` 贡献声明 owned collection；实际 `.events` 消费贡献单独取得权限，不继承 `.data` 或同包其他贡献的授权。

先按 [SDK README](../../../crates/runtime-extension-sdk/README.md) 构建真实 event worker，再从仓库根目录打包：

```bash
node scripts/node/plugin/cli.js package api/plugins/fixtures/acme.composition-c \
  --out tmp/plugin-composition-packages \
  --runtime-binary "$MANAGED_EVENT_WORKER_FIXTURE" --target x86_64-unknown-linux-gnu
```

CLI 把 SDK 二进制放到 manifest 的 `bin/worker.py`，后缀不表示 Python。安装、工作区分配后，由真实 managed schema owner 应用 owned collection，再启用消费。插件作者没有任意 SQL 权限。

向 `acme.composition-c.events` 分别授予 `event.subscribe`（`managed-event@1`，workspace）和 `plugin_data.owned.write`（`plugin-data@1`，`owned_collection` / `processed_models`）。副作用和消费 receipt 在同一 PluginData 事务提交；重复交付使用稳定 installation / workspace / contribution / event 身份去重，不以 generation 替代。外部副作用不在该原子保证内。

停用或撤销权限保留暂停的持久交付；重新授权不自动恢复，恢复需精确目标与当前权限。完整说明见[插件组合架构](../../../../docs/architecture/plugin-composition.md)。新增源码 fixture 尚待 Root 集中 QA。
