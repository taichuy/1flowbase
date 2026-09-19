---
memory_type: feedback
feedback_category: repository
topic: 本地私有配置永不提交，误推送后改写历史移除
summary: `.dsh/` 这类本机 harness / MCP 私有配置（含真实凭证）永不提交；已推送到远端时改写提交并 force push 清除，而不是叠加删除提交。
keywords:
  - gitignore
  - 本地私有配置
  - dsh
  - 凭证泄露
  - force push
  - amend
match_when:
  - 需要判断某个本地目录或配置文件是否应提交
  - 发现私有配置或凭证已经提交并推送到远端
  - 需要选择“新增删除提交”还是“改写提交并 force push”
created_at: 2026-09-19 10
updated_at: 2026-09-19 10
last_verified_at: 2026-09-19 10
decision_policy: direct_reference
scope:
  - .gitignore
  - .dsh
---
# 本地私有配置永不提交，误推送后改写历史移除

## 时间

`2026-09-19 10`

## 规则

- `.dsh/` 是本机 harness / MCP 私有配置目录（`mcp.json` 含 `Authorization` 凭证），永不提交；在 `.gitignore` 中显式声明 `.dsh/`。
- 这类文件已经推送到远端时，优先改写提交（`git commit --amend` 或 rebase）后用 `git push --force-with-lease` 清除远端历史，而不是再叠加一个“删除文件”的提交。
- 凭证一旦进入远端历史即视为泄露，必须轮换；改写历史不能撤回已经发生的暴露。
- 改写已推送分支前先确认该提交是否已推送，并把改写风险明确告知用户后由用户选择。

## 原因

- 叠加删除提交只能让工作区和后续检出干净，文件仍留在远端历史中可被检出，等于没移除。
- 本地私有配置常含真实凭证，泄露代价高于改写共享分支的同步成本。

## 适用场景

- 本地私有目录、凭证、harness / MCP 配置被误提交到已推送分支。
- 需要决定用新提交还是改写历史来撤下某个文件。
