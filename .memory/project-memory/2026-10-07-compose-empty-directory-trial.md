---
memory_type: project
topic: 单文件 Compose 空目录部署试验实例
summary: 用户要求保留可访问的本地 worktree 空目录试验，33100 端口运行最新 GHCR 镜像，配置和数据库在重建后保持一致。
created_at: 2026-10-07 11
updated_at: 2026-10-07 12
decision_policy: verify_before_decision
status: active
---

# 当前范围

Root 在 taichuy-dev 的 `/home/taichuy/git/git_worktree/compose-empty-directory`，分支 `codex/compose-empty-directory`、HEAD `f3c5c9411`，按用户要求验证单文件 Compose 部署以判断是否能省去 curl 获取额外配置。无固定截止日期；实例保留供用户访问，结束时间待用户决定。

当前试验入口是忽略目录下的 `deploy/empty-compose-builtin-trial/compose.yaml`。用户批准将内联初始化脚本移入镜像，随后批准改为 Shell。正式源码入口为 `docker/deployment/compose.yaml`，脚本为 `initialize.sh`、`api-start.sh`，已本地集成 dev；GHCR 镜像尚未发布这次 Shell 脚本。当前实例使用本地镜像 `1flowbase-api-server:builtin-shell-trial`，ID `5c9916b13c8b`。旧 `deploy/empty-compose-trial/` 和 `deploy/empty-compose-default-trial/` 数据保留，当前容器不再挂载它们。

# 资源和取证入口

- Compose project: `flowbase-empty-trial`；Web 端口 33100，数据库/API 不发布宿主端口。
- 访问：`http://10.225.25.240:33100` 或本机 `http://127.0.0.1:33100`。
- 初始账号 root；用户已确认管理员使用 `.env.example` 的固定默认值，由 Compose 设置并写入首次生成的配置，而非随机生成。密码文件位于 worktree 的 `tmp/test-governance/empty-compose-login.txt`，不在记忆中记录密码。
- 首次启动前部署目录仅含 Compose；init 容器生成 `config/.env`，通过 bind mount 持久化并设置权限，数据库与 API 启动入口读取。
- 实际拉取最新 GHCR 镜像；digest 记录在 `tmp/test-governance/empty-compose-images.log`。
- `/health`、浏览器认证请求和刷新后 `/api/console/me` 返回 200；force-recreate 后配置 hash 与数据库 system identifier 不变。
- 证据在 worktree 的 `tmp/test-governance/empty-compose-*`。未验收全部业务页面功能。
- 2026-10-07 11 新默认账号试验使用全新目录接管相同 project 和端口，默认凭据登录、浏览器刷新及认证接口返回 200；证据为 `tmp/test-governance/default-compose-*`。数据库密码和凭据加密主密钥仍随机生成。修改默认值不重置已有数据库账号。
- 2026-10-07 12 内置脚本版本通过 5 项初始化测试、32 项既有部署测试；最终本地镜像另用空目录在 33101 验证默认凭据登录，随后删除该临时 project 的容器与网络。33100 实例使用最终脚本镜像完成重建，配置 hash、数据库标识不变，登录仍成功。证据为 `tmp/test-governance/builtin-init-*`；未重新编译 Rust 或发布 GHCR。
- 2026-10-07 12 Shell 替换通过 29 项定向测试；33101 空目录 init PATH 不包含 Node 仍正常启动和登录，临时 QA 容器与网络已删除。33100 使用 Shell 本地镜像重建后配置 hash、数据库标识不变，登录与刷新认证通过；证据为 `tmp/test-governance/shell-init-*`。未重新编译 Rust 或发布 GHCR。

# 生命周期

用户明确要求访问试验实例，因此本轮保留容器和 worktree。后续处理前核验实例状态与端口；只能管理本试验 project，不操作现有开发数据库或其他 worktree。
