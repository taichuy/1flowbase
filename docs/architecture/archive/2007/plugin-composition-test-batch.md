# #2007 插件组合历史验收入口

本页保留原阶段命令与测试库存，不代表当前候选验收状态。当前契约见[插件组合](../../plugin-composition.md)。

## 官方 CLI 复现与证据

SDK 与 fixture 的构建、打包命令见 [SDK README](../../../../api/crates/runtime-extension-sdk/README.md) 和 [A fixture](../../../../api/plugins/fixtures/acme.composition-a/README.md)。使用仓库根目录、Linux、锁定依赖与实际 SDK worker，不能把旧 Python fixture 当成 typed Hook/event worker。

有限门禁 scope 为 `plugin-composition-2007`。Root 冻结并推送候选后，先核对远程分支 SHA，再以该分支名 dispatch；两个输入仍使用完整冻结 SHA。GitHub 的 dispatch ref 使用 branch / tag 名，不能用裸 SHA 代替：

```bash
: "${CANDIDATE_SHA:?请先设置 Root 已冻结的完整候选 SHA}"
candidate_remote_sha="$(git ls-remote --exit-code origin refs/heads/codex/plugin-composition-2007-reinstall | cut -f1)"
if [ "$candidate_remote_sha" != "$CANDIDATE_SHA" ]; then
  echo '远程分支与冻结候选不一致，停止 dispatch' >&2
  exit 1
fi
gh workflow run quality-gate.yml --repo taichuy/1flowbase \
  --ref codex/plugin-composition-2007-reinstall \
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

runner 校验 checkout / workflow SHA，串行构建真实 SDK examples 和 12 个 Rust test target，核对 50 个必需完整测试名，与既定回归过滤范围合并去重后逐项精确执行；另运行 4 组 Node 命令，其中测试命令显式使用 `--test-reporter=tap`，不依赖 Node 24 的终端展示默认值。TAP 证据必须有一致计划 / 结果、非零 tests、pass=tests、fail/cancelled/skipped/todo 全为零且进程退出码为零；单有 tests 数不能算通过。编译并行度复用 `scripts/node/testing/verify-runtime.js` 的 CPU 配置，CI 使用 runner 实际可用 CPU 数；Cargo 命令始终串行，每个 Rust 命令以 `--exact` 选择一项测试。缺名、零测试、忽略、失败、环境缺失都不能算通过。实际回归总数由编译产物 `--list` 决定，不预报通过数。报告及逐命令日志位于 `tmp/test-governance/2007`，工作流始终尝试上传 artifact，报告记录各 Node 组的独立状态、计数和实际 Cargo 并行度。dispatch 请求被拒且没有创建 run 时没有测试执行证据，不能计为一次验证或重试。AC / AUTH 映射是证据索引，最终验收由 Root 集中 QA 结算。


新增 IR01–03 在 `managed_reinstall_tests` 中经正式上传安装、真实 `MANAGED_EVENT_WORKER_FIXTURE`、Create owner、PostgreSQL 和 session/CSRF 治理 API 取证；IR04 在原必需 PostgreSQL fixture 中覆盖旧 metadata 已失去贡献的历史。50 项必需库存保留原 47 项与原 12 targets / 4 Node 组，所有新行为结论由冻结候选的集中 CI 给出，机械检查不代表测试通过。
