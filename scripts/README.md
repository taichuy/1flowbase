# 1flowbase Scripts

本目录收纳仓库级脚本。除特别说明外，命令都从仓库根目录执行。

## Runtime

- Node.js: `>=24.0.0`
- 前端依赖通过 `web/package.json` 管理，脚本内部需要前端工具时会从 `web/` 解析依赖。
- warning 与 coverage 产物统一写入 `tmp/test-governance/`。

## Git Helpers

### `node scripts/node/merge-current-to-main-latest.js [选项]`

将当前分支合并到 `main`，推送 `main`；成功后切到 `latest`，将 `main` 合并进 `latest` 并推送，最后切回执行前所在分支。
任意 git 步骤失败都会立即停止，并停留在失败发生时的分支/状态。

```bash
node scripts/node/merge-current-to-main-latest.js
node scripts/node/cli/merge-current-to-main-latest.js
```

默认使用 `origin`、`main`、`latest`，可用 `--remote`、`--main`、`--latest` 覆盖。
脚本默认要求工作区干净；确需允许未提交变更时可加 `--allow-dirty`。

## Dev Runtime

### `node scripts/node/dev-up.js [选项] [start|ensure|stop|status|restart]`

统一管理本地开发进程。默认动作是 `start`。

常用命令：

```bash
node scripts/node/dev-up.js
node scripts/node/dev-up.js --skip-docker
node scripts/node/dev-up.js restart --frontend-only
node scripts/node/dev-up.js restart --backend-only

node scripts/node/dev-up.js restart --frontend-only && node scripts/node/dev-up.js restart --backend-only

node scripts/node/dev-up.js status
node scripts/node/dev-up.js stop
```

说明：

- 默认管理前端、`api-server`、`plugin-runner`，并在全量动作下管理 `docker/docker-compose.middleware.yaml`。
- `--skip-docker` 只跳过 Docker 中间件，不影响前后端本地进程。
- `--frontend-only` 只管理前端。
- `--backend-only` 只管理 `api-server` 与 `plugin-runner`。
- 日志写入 `tmp/logs/`。
- pid 记录写入 `tmp/dev-up/pids/`。

## Test Scripts

### `node scripts/node/test.js <backend|contracts|frontend|scripts> [args]`

测试聚合入口。

```bash
node scripts/node/test.js backend
node scripts/node/test.js contracts
node scripts/node/test.js frontend fast
node scripts/node/test.js frontend full
node scripts/node/test.js scripts
```

### `node scripts/node/test-backend.js`

运行后端 Rust workspace 测试：

```bash
cargo test --workspace
```

实际并发参数由 `scripts/node/testing/verify-runtime.js` 的运行时配置控制。
默认编译 job 数和测试线程数都使用机器全部逻辑 CPU；同一 worktree 仍只运行一条 Cargo 命令。直接运行定向测试时复用同一配置，不写死 job 数：

```bash
CARGO_BUILD_JOBS="$(node scripts/node/testing/verify-runtime.js cargo-jobs)" \
  cargo test -p control-plane <test-filter> --lib
```

只有开发者主动排查资源问题时，才在 `.1flowbase.verify.local.json` 显式覆盖并发值；仓库本地默认不降载。

### `node scripts/node/cli/test-contracts.js`

运行跨消费者契约测试，当前聚焦模型供应商配置相关契约。

### `node scripts/node/test-frontend.js [fast|full]`

前端测试入口。

- `fast`: 运行 `web/app` 的快速测试，并写入 `tmp/test-governance/frontend-fast.log`。
- `full`: 运行前端 lint、workspace 测试、构建和样式边界检查。

`web/package.json` 中的 `test`、`test:fast`、`verify:full` 会调用该入口或相邻封装脚本。

### `node scripts/node/test-scripts.js [filter ...]`

运行 `scripts/node/**/_tests/*.js` 下的 Node 脚本测试。

```bash
node scripts/node/test-scripts.js
node scripts/node/test-scripts.js page-debug
node scripts/node/test-scripts.js verify-backend runtime-gate
```

### `node scripts/node/vite-lazy-deps-gate.js`

检查 `React.lazy(() => import(...))` 子图中的 lazy-only 裸 npm 依赖是否已写入
`web/app/vite.config.ts` 的 `optimizeDeps.include`，并检查 route-owned lazy import
是否有 `scripts/node/vite-lazy-deps-gate/manifest.json` smoke 覆盖。

```bash
node scripts/node/vite-lazy-deps-gate.js
node scripts/node/vite-lazy-deps-gate.js --smoke --web-base-url http://127.0.0.1:3100
```

默认只跑静态检查；`--smoke` 才启动 Playwright 并访问 manifest 中的路径，捕获 Vite dev
`504 Outdated Optimize Dep` 和 `Failed to fetch dynamically imported module`。

## Verification Scripts

### `node scripts/node/verify.js <backend|ci|coverage|repo> [args]`

验证聚合入口。

```bash
node scripts/node/verify.js backend
node scripts/node/verify.js coverage all
node scripts/node/verify.js repo
node scripts/node/verify.js ci
```

### `node scripts/node/verify-backend.js`

后端完整门禁：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -D warnings`
- `cargo test --workspace`
- `cargo check --workspace`

CI 可用 `core-libs`、`runtime-storage`、`apps` 分片；`test` 目标额外支持
`control-plane`、`api-server`、`plugin-runner` 包级分片。

### `node scripts/node/verify-coverage.js [frontend|backend|all]`

仓库覆盖率门禁。覆盖率摘要写入 `tmp/test-governance/coverage-summary.log`，后端覆盖率需要本地安装 `cargo-llvm-cov`。

### `node scripts/node/verify-repo.js`

仓库级验证组合：

- gate router 提示：根据 changed files 输出非阻塞相关门禁建议；提交前 hook 使用 staged changes
- repo hygiene 审计：废弃标记、弱断言、重复测试标题、文件/目录压力
- i18n hygiene 审计：locale 文件名、key 对齐、owner 内重复 key/value、未引用前端 key warning
- 脚本测试
- 契约测试
- 前端 full gate
- 后端完整门禁

### `node scripts/node/verify-ci.js`

CI 组合入口，当前执行 `verify-repo`、`verify-backend-consistency` 与 `verify-coverage all`。

## Frontend Tooling

### `node scripts/node/check-style-boundary/cli.js <component|page|file|all-pages> [target]`

检查样式影响边界。

```bash
node scripts/node/check-style-boundary/cli.js component component.account-popup
node scripts/node/check-style-boundary/cli.js page page.home
node scripts/node/check-style-boundary/cli.js file web/app/src/styles/global.css
node scripts/node/check-style-boundary/cli.js all-pages
```

脚本会按需确保前端开发服务可用，并通过 Playwright 读取样式边界场景。

### `node scripts/node/page-debug/cli.js [snapshot|open|login] <route-or-url> [选项]`

页面调试和取证脚本。

```bash
node scripts/node/page-debug/cli.js /workspace
node scripts/node/page-debug/cli.js open /workspace --headless false
node scripts/node/page-debug/cli.js login --account <account> --password <password>
```

默认产物写入 `tmp/page-debug/<timestamp>/`，包括截图、控制台日志、元数据和 DOM snapshot。

`page-debug` 与 `tooling.js api-debug` 使用各自隔离的临时 console session：快照、请求和失败路径结束时自动注销，`open` 模式在浏览器断开时注销。不要在临时 Playwright / Node 脚本中直接调用 `/api/public/auth/sign-in`；确需自定义浏览器流程时，复用 `scripts/node/page-debug/auth.js` 暴露的临时 session owner，并在 `finally` 调用 `dispose()`。

常用选项：

- `--web-base-url <url>`: 默认 `http://127.0.0.1:3100`
- `--api-base-url <url>`: 默认 `http://127.0.0.1:7800`
- `--out-dir <dir>`: 指定输出目录
- `--wait-for-selector <selector>`: 等待指定元素
- `--wait-for-url <url>`: 等待目标 URL

### `node scripts/node/cli/runtime-gate.js <page-debug args>`

运行时页面检查入口，本质上是对 `page-debug` 的门禁封装。

## Workspace Tools

### `node scripts/node/cli/mock-ui-sync.js [选项]`

从 `web/` 重建 mock UI 工作区。

```bash
node scripts/node/cli/mock-ui-sync.js
node scripts/node/cli/mock-ui-sync.js --source web --target tmp/mock-ui --port 3210
```

说明：

- 默认清空并重建 `tmp/mock-ui/`。
- 默认把 mock 副本前端端口改成 `3210`。
- 会排除 `node_modules`、`dist`、`coverage`、`.turbo`、`.vite`。

### `node scripts/node/cli/claude-skill-sync.js [选项]`

将 `.agents/skills` 同步为 Claude 可识别的 `.claude/skills/<name>/SKILL.md` 结构。

```bash
node scripts/node/cli/claude-skill-sync.js
node scripts/node/cli/claude-skill-sync.js --source .agents/skills --target .claude/skills
```

### `node scripts/node/cli/acp-claude-smoke.js [选项]`

通过 ACP adapter 启动 Claude Code，发送一轮 prompt，并把 `session/update`、raw SDK message 和 stderr 证据写入 `tmp/test-governance/acp-claude-smoke/`。
默认要求同时出现 `agent_thought_chunk` 和 `agent_message_chunk`，用于验证 Anthropic-compatible reasoning 是否能被 Claude Code ACP 投射为思考 chunk。

```bash
node scripts/node/cli/acp-claude-smoke.js --model 1flowbase
node scripts/node/cli/acp-claude-smoke.js --model 1flowbase --out-dir tmp/test-governance/issue-922
node scripts/node/cli/acp-claude-smoke.js --allow-missing-thought
```

### `node scripts/node/verify-state-protocols.js [选项]`

固定状态协议回归门禁。按顺序运行 ACP smoke 脚本单测、`api-server anthropic_` 协议投影测试、后端 `dev-up ensure`，最后用真实 Claude Code ACP smoke 验证 `agent_thought_chunk` 与 `agent_message_chunk`。

```bash
node scripts/node/verify-state-protocols.js
node scripts/node/verify-state-protocols.js --model 1flowbase --out-dir tmp/test-governance/state-protocols/acp-claude
node scripts/node/verify-state-protocols.js --skip-live-acp
```

### `node scripts/node/tooling.js <command> [args]`

工具聚合入口，支持：

- `check-style-boundary`
- `claude-skill-sync`
- `i18n-hygiene`
- `mock-ui-sync`
- `page-debug`
- `repo-hygiene`
- `runtime-gate`

### `node scripts/node/tooling.js repo-hygiene [--max-findings <n>]`

工程卫生审计入口。默认写入 `tmp/test-governance/repo-hygiene.json`，覆盖：

- 历史债务类标记
- `test.only`、跳过测试和弱断言
- 重复测试标题
- 超大文件和目录文件数压力

当前只有会改变 CI 测试语义的 `test.only` 类命中阻塞；历史债先以 warning 形式进入 QA 证据。

### `node scripts/node/tooling.js i18n-hygiene [--max-findings <n>]`

多语言资源审计入口。默认写入 `tmp/test-governance/i18n-hygiene.json`，覆盖：

- 前端 `zh-CN.json / en-US.json` 与插件 `zh_Hans.json / en_US.json` 文件名规则
- 同一 `i18n/` owner 下 locale key 对齐
- JSON 重复 key
- 同 owner、同 locale 内重复 value 阻塞
- 前端 `i18n/` key 无静态代码引用 warning
- 可用 `--include-cross-owner-warnings` 额外查看跨 owner 重复 key / value advisory warning

## MCP Publishing

### `node scripts/node/export-mcp-instance-to-official.js --instance-id <id> --target <absolute-path>`

将一个 MCP 实例同步到官方插件源码目录。脚本使用临时 owner session，自动递增目标 Bundle 的 patch 版本，调用官方仓库校验器，并原子替换目标目录；不会 commit 或 push。

官方发布导出只保留内置静态 API、内置数据表 CRUD 和上游 MCP Proxy Tool；自动排除 `/api/ex/*` Workflow 与工作区自建数据表生成的 CRUD Tool。完整参数和失败保护说明见 [`scripts/node/export-mcp-instance-to-official/README.md`](node/export-mcp-instance-to-official/README.md)。

```bash
node scripts/node/export-mcp-instance-to-official.js \
  --instance-id 1flowbase \
  --target /home/taichuy/git/1flowbase-official-plugins/mcp/@taichuy/1flowbase_zh_hans \
  --api-base-url http://127.0.0.1:7800
```

## Version Tools

### `node scripts/node/cli/bump-version.js [0|1|2|patch|minor|major] [--dry-run]`

一键升级仓库自有组件版本号。脚本从根目录 `VERSION` 读取当前仓库版本，先计算一个目标版本，再统一写入自有前端 package、Rust 后端 package 和 `api/Cargo.lock`。

默认执行 `patch` 升级，也可以指定升级类型或直接锁定到目标版本。数字别名为：`0` = `patch`，`1` = `minor`，`2` = `major`。

```bash
node scripts/node/cli/bump-version.js --dry-run
node scripts/node/cli/bump-version.js patch
node scripts/node/cli/bump-version.js minor
node scripts/node/cli/bump-version.js major
node scripts/node/cli/bump-version.js 1
node scripts/node/cli/bump-version.js 2
node scripts/node/cli/bump-version.js --to 0.3.0
```

说明：

- 会更新根目录 `VERSION`、自有前端 package、Rust 后端 package，以及 `api/Cargo.lock` 中对应自有 package 版本。
- `--dry-run` 只打印将要修改的文件和版本变化，不写入文件。
- 不修改插件 manifest、Docker env 文件或第三方镜像 tag。
- `--to <x.y.z>` 不能和 `0`、`1`、`2`、`patch`、`minor`、`major` 同时使用。

### `node scripts/node/container-image-security/verify-version.js <component> <vX.Y.Z>`

校验容器镜像 tag 是否和组件 manifest 版本一致。

```bash
node scripts/node/container-image-security/verify-version.js print-tag web
node scripts/node/container-image-security/verify-version.js web v0.3.0
node scripts/node/container-image-security/verify-version.js api-server v0.3.0
node scripts/node/container-image-security/verify-version.js plugin-runner v0.3.0
```

支持组件：`web`、`api-server`、`plugin-runner`。

## Plugin CLI

### `node scripts/node/plugin/cli.js <command> [options]`

宿主侧插件脚手架和打包工具。

```bash
node scripts/node/plugin/cli.js init <plugin-path>
node scripts/node/plugin/cli.js demo init <plugin-path>
node scripts/node/plugin/cli.js demo dev <plugin-path> --port 4310
node scripts/node/plugin/cli.js package <plugin-path> --out ./dist
```

说明：

- `init` 生成 model provider runtime extension 基础源码结构。
- `demo init` 在插件目录下生成本地 demo 页面和辅助脚本。
- `demo dev` 用 Node 内建静态服务启动 `demo/`，默认地址 `http://127.0.0.1:4310`。
- `package` 生成过滤 `demo/`、`scripts/`、`target/` 后的 `.1flowbasepkg` 安装产物，并返回 sha256 元数据。
- 可通过 `--runtime-binary`、`--target`、`--signing-key-pem-file`、`--signing-key-id`、`--issued-at` 写入 runtime binary 和官方签名元数据。

当前 demo dev 仍是本地 scaffold，不代表真实 `plugin-runner` debug runtime 已经打通。

## Artifact Cleanup

### `node scripts/node/clean-build-cache/cli.js [all|backend|frontend] [选项]`

停止 `api-server` 与 `plugin-runner`，然后清理本地构建缓存。默认范围是 `all`，会真实删除。

```bash
node scripts/node/clean-build-cache/cli.js --dry-run
node scripts/node/clean-build-cache/cli.js
node scripts/node/clean-build-cache/cli.js --backend-only
node scripts/node/clean-build-cache/cli.js --frontend-only
```

范围：

- `all`: 清理 `api/target`、`web/.turbo`、`web/app/.turbo` 和 `web/app/dist`。
- `backend` / `--backend-only`: 仅清理 `api/target`。
- `frontend` / `--frontend-only`: 仅清理前端构建缓存。
- `--dry-run`: 只打印将要清理的路径，不停止进程、不删除文件。

### `node scripts/node/clean-artifacts/cli.js [profile] [--apply]`

查看或清理本地临时产物。默认 profile 是 `status`，只打印体积，不删除。

```bash
node scripts/node/clean-artifacts/cli.js
node scripts/node/clean-artifacts/cli.js light
node scripts/node/clean-artifacts/cli.js light --apply
node scripts/node/clean-artifacts/cli.js backend-cache --apply
node scripts/node/clean-artifacts/cli.js all --apply
node scripts/node/clean-artifacts/cli.js deep --apply
```

profile：

- `status`: 查看 `api/target`、coverage、前端构建和 `tmp` 目录体积。
- `light`: 清理前端构建缓存和运行态 `tmp` 目录。
- `backend-cache`: 清理 Cargo incremental、llvm-cov target 和 target/tmp，保留 debug/deps。
- `all`: 执行 `light + backend-cache`。
- `deep`: 清理 `light + api/target`，下一次后端编译会明显变慢。

## Runtime Helpers

- `scripts/node/cli/exec-with-real-node.sh`: 从前端包脚本调用仓库 Node 脚本时，确保使用真实 Node runtime。
- `scripts/node/testing/*`: 脚本共享的运行时配置、warning capture、coverage threshold 和 Node runtime 解析工具。
## docker

下面的命令不会安装 Docker。部署脚本只会先检查本机是否已经有可用的 Docker/Compose 环境，然后把 `docker/` 目录拉到当前目录，复制 `docker/.env.example` 为 `docker/.env`。随后脚本会进入交互配置，空输入保留提示中的当前值，再让你选择是否拉取镜像、是否启动容器。脚本输出保持英文，避免终端编码问题。

#### Shell

```bash
curl -fsSL https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/shell/docker-deploy.sh | sh
```

#### PowerShell

```powershell
irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex
```

#### Windows CMD

```bat
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex"
```

交互配置项包括：

- `POSTGRES_PASSWORD`
- `BOOTSTRAP_ROOT_ACCOUNT`
- `BOOTSTRAP_ROOT_PASSWORD`
- `API_PROVIDER_SECRET_MASTER_KEY`
- `WEB_PORT`
- 官方插件 GitHub raw 下载加速：提示 `Use CN GitHub plugin download accelerator? [y/N]` 时默认不启用；输入 `y` 后继续填写 `API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL`，地址提示中直接回车会使用默认 `https://gh-proxy.com/`。
- 官方插件签名强制校验：默认 `API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true`；只有在明确接受第三方 / 未签名插件风险的自托管环境中才设置为 `false`。关闭后仍保留插件包 `sha256` checksum 校验，未验证包会标记为 `unverified`。

如果 `API_PROVIDER_SECRET_MASTER_KEY` 仍是默认占位值，脚本会自动生成随机 key 写入 `docker/.env`，不需要首次部署时手动填写。

之后脚本会继续询问：

- `Pull Docker images? [y/N]`
- `Start 1flowbase now? [y/N]`

拉取或启动前，脚本会检测有效 Docker 平台。官方镜像支持 `linux/amd64` 和 `linux/arm64`，Docker 会自动拉取匹配 manifest；如果当前 tag 缺少本机平台 manifest，脚本会提前失败并提示重新发布多架构镜像。临时需要强制 x86 镜像时，可以设置 `DOCKER_DEFAULT_PLATFORM=linux/amd64`。

如果选择暂时不启动，之后可以手动执行：

```bash
cd docker
docker compose pull
docker compose up -d
```

非交互环境可以使用 `--db-password`、`--root-account`、`--root-password`、`--provider-secret`、`--web-port`、`--plugin-github-proxy-url`、`--official-plugin-signature-required`、`--pull`、`--start`、`--no-pull`、`--no-start` 和 `--non-interactive` 控制行为。也可以用 `FLOWBASE_OFFICIAL_PLUGIN_GITHUB_PROXY_URL` 预填官方插件 GitHub raw 下载代理，用 `FLOWBASE_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=false` 显式关闭官方插件签名强制校验。
