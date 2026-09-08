# 受管插件作者 SDK

SDK 提供 typed Hook / event worker 协议与作者侧 client。Host 负责可信 context、权限、调用 scope 和进程生命周期；SDK 不提供 SQL、数据库连接、宿主 registry 或无限制 RPC。plugin → Host 回调不能自行声明可信 installation / workspace / actor，Host → worker 的 typed context 由 Host 注入。

当前示例覆盖只读 Create Hook 与 committed / processed 事件。贡献必须经正式包安装、工作区分配、独立授权与执行绑定；manifest 声明不是授权。同包贡献不共享权限。事件消费的 owned-write 必须授予实际消费贡献。

## 构建实际 worker

在仓库根目录、Linux 上执行：

```bash
export CARGO_TARGET_DIR="$(pwd)/tmp/quality-gate-cache/plugin-composition-2007/target"
cargo build --locked --manifest-path api/Cargo.toml -p runtime-extension-sdk --example managed_hook_worker
cargo build --locked --manifest-path api/Cargo.toml -p runtime-extension-sdk --example managed_event_worker
export MANAGED_HOOK_WORKER_FIXTURE="$CARGO_TARGET_DIR/debug/examples/managed_hook_worker"
export MANAGED_EVENT_WORKER_FIXTURE="$CARGO_TARGET_DIR/debug/examples/managed_event_worker"
```

这两个 example 来自 `src/_tests/managed_hook_worker.rs` 与 `managed_event_worker.rs`。验收使用绝对路径和真实构建产物，缺少环境变量不能静默跳过。示例是有限 fixture，不是任意接口的自动开放承诺。

## 正式包

```bash
node scripts/node/plugin/cli.js package api/plugins/fixtures/acme.composition-a \
  --out tmp/plugin-composition-packages \
  --runtime-binary "$MANAGED_HOOK_WORKER_FIXTURE" --target x86_64-unknown-linux-gnu
```

CLI 将可执行文件放入 manifest 指定的 `bin/worker.py`；这个历史文件名不代表 Python，正式复现使用上面的 SDK ELF 二进制。事件变体及 B/C 见 [A](../../plugins/fixtures/acme.composition-a/README.md)、[B](../../plugins/fixtures/acme.composition-b/README.md)、[C](../../plugins/fixtures/acme.composition-c/README.md)。

授权、幂等限制、epochs、退休、资源预算与关闭顺序见[插件组合架构](../../../docs/architecture/plugin-composition.md)。当前新增源码 fixture 尚待 Root 集中 QA，构建与打包命令是复现说明，不是已执行证据。
