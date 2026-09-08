# 组合插件 A

`manifest.yaml` 在一个安装身份下提供 `.first` / `.second` 两个 Create.before 贡献和声明式 `.data` owned collection。两个可执行贡献分别授权，用于发现同包权限合并错误。Before 只读且可否决，不能改变 Create 输入或核心拒绝。

先按 [SDK README](../../../crates/runtime-extension-sdk/README.md) 构建两个真实 worker。在仓库根目录打包原始 Hook 变体：

```bash
node scripts/node/plugin/cli.js package api/plugins/fixtures/acme.composition-a \
  --out tmp/plugin-composition-packages \
  --runtime-binary "$MANAGED_HOOK_WORKER_FIXTURE" --target x86_64-unknown-linux-gnu
```

目录内旧 Python worker 属于早期协议 fixture；typed Hook / event 复现使用 SDK example。`bin/worker.py` 只是 manifest 中保留的入口名称，CLI 在该位置放置实际二进制。

## 事件变体

`event-manifest.yaml` 的 `.events` 贡献以 `publish` handler 订阅 `model_definition.committed@v1`，发布 `acme.composition-a.processed@1`。使用临时目录，不改写原始 manifest：

```bash
variant_dir="$(mktemp -d)"
cp api/plugins/fixtures/acme.composition-a/event-manifest.yaml "$variant_dir/manifest.yaml"
node scripts/node/plugin/cli.js package "$variant_dir" \
  --out tmp/plugin-composition-packages \
  --runtime-binary "$MANAGED_EVENT_WORKER_FIXTURE" --target x86_64-unknown-linux-gnu
rm -rf "$variant_dir"
```

安装并分配工作区后，分别向 `acme.composition-a.events` 授予 `event.subscribe`、`event.publish`（`managed-event@1`，workspace scope）。Host 在当前授权锁下提交稳定派生 E2。B/C 使用独立贡献授权，以原子 receipt / effect 消费。因果 ID 不代替授权；这不保证外部服务 exactly-once。

完整治理与有限门禁见[架构说明](../../../../docs/architecture/plugin-composition.md)。源码 fixture 尚待 Root 集中 QA，本文不声明已通过。
