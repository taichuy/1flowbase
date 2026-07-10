# Quality Gate Watch

## Purpose

用于周期性“质量门禁检测”或用户要求确认仓库质量门禁状态时。先判断操作者权限，再选择 GitHub 闭环或本地脚本取证。工作语言用中文，所有结论证据优先。

## Scenario Selection

| 场景 | 使用方式 | 可下的结论 |
| --- | --- | --- |
| 仓库管理者或维护者，有 GitHub Actions、artifact、issue 权限 | 用 GitHub workflow run、`test-governance-artifacts`、quality-gate issue 闭环 | 可确认远端门禁是否通过，并评论 / 关闭质量 issue |
| 无权限贡献者，无 issue 或 Actions 权限 | 用本地脚本运行对应 gate，保存 `tmp/test-governance/` 证据 | 只能确认“本地脚本在当前环境的结果”，不能宣称远端门禁已通过或关闭 issue |

## Manager GitHub Flow

默认从 `latest` 分支开始，除非用户明确指定其他分支。开始前读取 `AGENTS.md`、`.memory/AGENTS.md`、`.memory/user-memory.md`，并遵守 `qa-evaluation` 的证据规则。不要运行本地全量 quality gate 来替代 GitHub Actions；完整门禁以远端 Actions 和 artifact 为准。

检查顺序：

1. 运行 `git status --short --branch` 和 `git log -1 --oneline --decorate`，确认本地分支与提交。
2. 查找最新相关 quality-gate issue，优先 `latest` 分支报告；如果旧 issue 仍 open，等新的有效通过报告出现后再处理。
3. 检查 `latest` 分支的 GitHub Actions，重点看 `verify` 和手动 `manual quality gate` workflow。
4. 下载或读取 `tmp/test-governance/quality-gate-report.json`。通过条件必须同时满足：workflow conclusion 为 `success`、artifact `status=passed`、`exitCode=0`、`warningFiles=[]`。
5. 如果 gate 没有在 `latest` 上运行，先修 workflow/action，再补聚焦测试。重点文件通常是 `.github/actions/quality-gate/action.yml`、`.github/workflows/verify.yml`、`.github/workflows/quality-gate.yml`。
6. workflow/action 变化要用 `node scripts/node/test-scripts.js github-quality-gate` 或等价定向测试验证；不要靠肉眼检查。
7. 推送到目标分支后等待 GitHub Actions 完成，再下载 artifact 复核 JSON，最后再说 pass/fail。
8. 通过后，在最新 quality issue 评论证据并关闭；旧 open issue 在新有效 pass 出现后关闭。

评论证据至少包含：run URL 或 run id、workflow、branch、commit、issue number、run conclusion、artifact `status`、`exitCode`、`warningFiles`。

## Contributor Local Flow

无 GitHub 权限时，目标是给出可复查的本地质量证据，不做远端状态承诺。

推荐顺序：

1. 运行 `git status --short --branch` 和 `git log -1 --oneline --decorate`，说明当前分支、commit、是否有未提交改动。
2. 根据问题范围选择脚本：
   - 仓库级 CI 模拟：`node scripts/node/verify-ci.js`
   - 仓库级可合入基线：`node scripts/node/verify-repo.js`
   - workflow / quality-gate 脚本本身：`node scripts/node/test-scripts.js github-quality-gate`
   - 更窄范围按 `repo-quality-gates.md` 选择。
3. 产物统一检查 `tmp/test-governance/`，特别是对应 log、warning log、coverage summary 或 quality-gate report。
4. 交付时明确写“本地验证结果”，列出命令、退出码、关键产物路径和失败摘要。不要写“GitHub 质量门禁已通过”，除非已经拿到远端 Actions 和 artifact 证据。

## Workflow Reference

当前仓库的质量门禁自动化入口：

- `.github/workflows/verify.yml`：`pull_request`、`main` 和 `latest` push 触发，调用本地 quality-gate action。
- `.github/workflows/quality-gate.yml`：手动 quality gate，`target_branch` 默认 `latest`，可选 `latest` / `main`。
- `.github/actions/quality-gate/action.yml`：复用 action，实际执行 `node scripts/node/cli/github-quality-gate.js`。
- `scripts/node/cli/github-quality-gate.js`：生成 `quality-gate.latest.log`、`quality-gate-report.md`、`quality-gate-report.json`，有 token 时发布 GitHub issue。

## Hard Stops

- 没有 GitHub artifact JSON 时，不得声称远端门禁通过。
- 没有 issue 权限时，不得承诺已评论或关闭 quality-gate issue。
- 本地脚本失败时，先按失败日志定位最小修复；需要产品或策略决策时写入 `.memory/todolist`，不要猜。
- `warning` 与 `coverage` 产物必须留在 `tmp/test-governance/`。
