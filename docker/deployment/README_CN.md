# 单文件 Docker 部署

[English](README.md)

将 `compose.yaml` 保存到空目录，运行：

```bash
docker compose up -d
```

访问 `http://localhost:3100`。初始账号为 `root`，初始密码为
`change-me-root-password`，登录后修改密码。该配置需要 API 镜像内置
`/usr/local/bin/1flowbase-initialize` 和 `/usr/local/bin/1flowbase-api-start`；
本配置发布前的旧镜像不包含这些脚本。

初始化服务执行 API 镜像内置的 POSIX Shell 脚本，不依赖 Node.js，创建挂载目录和 `config/.env`，随机生成独立的
数据库密码与加密主密钥，然后退出。PostgreSQL 和 API 读取保存的配置，重建容器
不会重置配置。配置应与数据库和上传文件一起保留；已有数据库缺少原配置时启动会失败。

首次启动前，可选的 Compose `.env`（与 `compose.yaml` 同目录）可以配置
`WEB_PORT`、`FLOWBASE_API_SERVER_VERSION`、`FLOWBASE_WEB_VERSION`、
`BOOTSTRAP_ROOT_ACCOUNT`、`BOOTSTRAP_ROOT_PASSWORD`、`POSTGRES_PASSWORD` 和
`API_PROVIDER_SECRET_MASTER_KEY`。初始化参数仅在创建 `config/.env` 时生效，修改它们
不会重置已有账号或轮换已有密钥。默认使用 HTTP；浏览器入口使用 HTTPS 时设置
`API_COOKIE_SECURE=true`，包括外层 nginx 终止 TLS、内部仍使用 HTTP 的情况。
浏览器客户端从不同来源访问 API 时，配置 `API_ALLOWED_ORIGINS`。

`config/.env` 包含敏感信息，不要提交，也不要通过删除它重置账号。
本配置用于内置 PostgreSQL 的全新部署。已有部署、外部 PostgreSQL 和便携备份恢复
继续使用 [Docker 指南](../README.md) 中的部署脚本与配置。

## Nginx 配置与升级

Web 镜像启动时通过目录挂载 `./web/nginx:/config/nginx` 自动导出配置，不需要先创建配置文件：

- `web/nginx/nginx.default.conf`：当前镜像的默认模板，每次启动刷新，包含 `/api/` 与 `/v1/` 的 WebSocket 转发。不要直接编辑此文件。
- `web/nginx/nginx.conf`：可选的完整自定义配置，默认不创建；存在时优先使用，重启、重建及升级均不覆盖。升级后需与默认模板比较。

不创建自定义文件时，镜像升级自动应用新的默认配置。需要自定义时，在部署目录执行：

```bash
sudo cp web/nginx/nginx.default.conf web/nginx/nginx.conf
# 使用 sudo 编辑 web/nginx/nginx.conf，然后检查并重建 Web
docker compose run --rm --no-deps web nginx -t &&
docker compose up -d --no-deps --force-recreate web
```

Web 启动时会验证 Nginx 配置，错误会写入 `docker compose logs web` 并阻止启动。
移走自定义 `web/nginx/nginx.conf` 后重建 Web，即恢复当前镜像默认配置；不用删除整个配置目录。

### 从旧部署迁移

此机制需要包含 `40-flowbase-nginx.sh` 的新版 Web 镜像；仅更新 Compose 而保留旧镜像不会导出模板。
升级 Web 镜像时同步更新 Compose。若原配置包含
`./web/nginx.conf:/etc/nginx/conf.d/default.conf:ro`，将该条替换为
`./web/nginx:/config/nginx`，保留 external-npm 等其他挂载。
确有自定义规则时，先确认旧路径是普通文件，再将它复制到 `web/nginx/nginx.conf`；
没有自定义规则则直接使用镜像默认配置。旧路径若被 Docker 误建成目录，先移走备份，不要把目录当配置复制。

新镜像仍支持旧的直接文件挂载，并在日志提示迁移，但旧挂载会覆盖镜像的新默认配置。
本次 SSH WebSocket 修复不会自动进入用户自定义配置：需补齐 `/api/` 内的
`proxy_set_header Upgrade $http_upgrade;` 与
`proxy_set_header Connection $connection_upgrade;`，或移除自定义覆盖以使用新默认值。

### 公网静态站点与 FRP

若公网服务器单独托管前端编译包，仅将 API 经 FRP 转发到 Docker 的 `3100`，公网 Nginx 也必须配置页面路由回退：

```nginx
location / {
    try_files $uri $uri/ /index.html;
}
```

该规则放在公网域名的 `server` 内，`root` 指向实际包含 `index.html` 的编译包目录；已有 `location /` 时修改原规则，不重复添加。
`/settings/...` 和 `/mcp/authorize` 是前端路由，不能按静态文件查找或误代理到业务 API。
Docker 内的回退规则无法修复公网静态站点的 404。
公网 API 代理也须保留原请求路径、Host、HTTPS 协议信息及 WebSocket 升级头；
MCP 使用的 `/.well-known/oauth-*` 发现路径需要转发，不能只由公网证书验证目录处理。
公网编译包与 Docker Web 镜像应保持版本一致，避免两份前端资源不一致。

## 自动同步国内镜像

`aliyun image sync` 是独立的 GitHub Actions 工作流。它监听 `latest` 分支的
`container images` 工作流成功结束；同步失败不会改变 GHCR 或 GitHub Release
发布结果。新增同步工作流及其脚本需要进入仓库默认分支，`workflow_run` 才会自动触发。

在仓库 Settings → Secrets and variables → Actions 设置：

| 名称 | 类型 | 内容 |
| --- | --- | --- |
| `ACR_SYNC_ENABLED` | Variable | `true` 启用，其他值停用 |
| `ACR_REGISTRY` | Variable | 实例公网域名，不含 `https://` |
| `ACR_API_REPOSITORY` | Variable | 后端 `命名空间/仓库名` |
| `ACR_WEB_REPOSITORY` | Variable | 前端 `命名空间/仓库名` |
| `ACR_POSTGRES_REPOSITORY` | Variable | PostgreSQL `命名空间/仓库名` |
| `POSTGRES_SOURCE_IMAGE` | Variable | `docker.io/library/postgres:18-alpine`，也可固定 18 的小版本 |
| `ACR_USERNAME` | Variable | 登录账号 |
| `ACR_PASSWORD` | Secret | Registry 登录密码 |

同步仅处理来源任务实际发布成功的组件，使用各组件自己的版本号，并核对镜像的
发布提交。前后端使用不同目标仓库，各自发布 `v版本` 和 `latest` 标签。PostgreSQL 单独镜像
官方 18 Alpine，发布当前 `18.小版本-alpine` 和 `18-alpine`；不使用会跨主版本的
`latest` 标签。复制保留 `linux/amd64`、`linux/arm64` 运行镜像及各架构摘要，不重新构建。
阿里云个人版不支持 OCI 构建证明，因此目标镜像列表排除 attestation，顶层摘要
会变化；验收逐一比较两种运行架构的镜像摘要。同步使用固定版本和校验和的
regctl v0.11.6，仅用于 CI 镜像复制，不增加应用依赖。
只有 GHCR 的当前 `latest` 仍指向此次镜像时才更新对应国内 `latest`。

失败后可以重跑同步工作流；也可以选择 Run workflow，填写成功的
`container images` 运行 ID（来源任务必须属于本仓库 `latest` 分支）。候选镜像未
推广到正式版本、来源镜像已被另一提交覆盖或运行架构摘要无法保留时，同步会失败。
校验记录位于该次同步的 `aliyun-image-sync-*` artifact。

## 国内镜像一键部署

将 `compose.aliyun.yaml` 保存到空目录，重命名为 `compose.yaml`，运行 `docker compose up -d`。
默认前后端分别拉取阿里云独立仓库的 `latest`，数据库拉取 `taichuy/postgres:18-alpine`。
API 与 init 使用同一份后端镜像。每次启动都会检查远端镜像，已有配置和数据库保持不变。

可选的 `.env` 可以设置 `FLOWBASE_REGISTRY`、`FLOWBASE_API_REPOSITORY`、
`FLOWBASE_WEB_REPOSITORY`、`FLOWBASE_POSTGRES_REPOSITORY`，以及三个组件各自的版本变量
`FLOWBASE_API_SERVER_VERSION`、`FLOWBASE_WEB_VERSION`、`FLOWBASE_POSTGRES_VERSION`。
绑定版本示例为 `v0.5.6`、`v0.5.6`、`18.6-alpine`。服务端口、初始账号等配置沿用上文。

[中文 Wiki 教程](https://github.com/taichuy/1flowbase/wiki/Docker-Aliyun-Deployment-CN)
提供完整单文件配置、升级与持久化说明。私有仓库需要在部署机器上先登录 Registry。
