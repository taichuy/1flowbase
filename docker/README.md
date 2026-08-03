# 1flowbase Docker

## 仅启动中间件

```powershell
Copy-Item .\middleware.env.example .\middleware.env
docker compose -f .\docker-compose.middleware.yaml up -d
```

当前默认本地中间件只包含 PostgreSQL。Redis 不作为默认依赖；后续通过 Redis HostExtension / infra plugin 接入。RustFS 不作为默认依赖；对象存储由后台文件存储配置选择，默认使用本地存储。

内置数据库默认使用 PostgreSQL 18。PostgreSQL 18 官方镜像把持久化根目录改为 `/var/lib/postgresql`，实际数据位于版本化子目录。若挂载目录根部存在 PostgreSQL 17 或更早版本的 `PG_VERSION`，容器会拒绝初始化，避免旧数据被忽略后静默启动为空库。主版本升级必须先保留原目录，并通过 `pg_dump` / `pg_restore` 或 `pg_upgrade` 显式迁移；不能只修改镜像 tag。

默认映射端口：

- PostgreSQL: `35432`

`docker/sandbox/config.yaml` 是默认 / reference sandbox backend 的配置模板，后续如果要接代理、限网或更严格隔离，可以直接在这里扩展。它当前更适合作为可选参考执行后端，而不是普通 workflow 开发的硬前置依赖。

## 使用 GHCR 镜像启动整套服务

新机器可以直接启动：

```bash
cd docker
cp .env.example .env
docker compose up -d
```

如果本机只有旧版 Compose，也可以把 `docker compose` 替换为 `docker-compose`。

本地快速体验可以使用开发 compose，它保留本地默认密码并以 development 模式启动：

```bash
cd docker
docker compose -f docker-compose.dev.yaml up -d
```

生产部署或需要固定配置时，使用统一配置模板：

```bash
cd docker
cp .env.example .env
```

编辑 `.env`，重点修改：

- `POSTGRES_PASSWORD`
- `API_PROVIDER_SECRET_MASTER_KEY`
- `BOOTSTRAP_ROOT_PASSWORD`

生产 compose 对这些敏感配置使用必填校验；缺少 `.env` 或关键 secret 时会直接失败，不再使用默认密码启动。然后启动：

```bash
docker compose up -d
```

### 使用外部 PostgreSQL

一键部署脚本会先询问数据库模式：直接回车或选择 `1` 使用内置 PostgreSQL；选择 `2` 使用外部 PostgreSQL。外部模式会继续询问 PostgreSQL 的 host/IP、端口、database、账号、密码和 `sslmode`，并写入 `API_DATABASE_URL`。

也可以在 `docker/.env` 中手动配置：

```env
DATABASE_MODE=external
EXTERNAL_POSTGRES_HOST=db.internal.example
EXTERNAL_POSTGRES_PORT=5432
EXTERNAL_POSTGRES_DB=1flowbase
EXTERNAL_POSTGRES_USER=flowbase
EXTERNAL_POSTGRES_PASSWORD=change-me
EXTERNAL_POSTGRES_SSLMODE=prefer
API_DATABASE_URL=postgres://flowbase:change-me@db.internal.example:5432/1flowbase?sslmode=prefer
```

外部模式不启动 compose 内置的 `db` 服务：

```bash
docker compose -f docker-compose.external-db.yaml pull
docker compose -f docker-compose.external-db.yaml up -d
```

注意：外部数据库地址必须能从 Docker 容器网络访问。`127.0.0.1` 在容器内通常指向容器自身，不是宿主机或内网数据库；如果数据库在宿主机上，按 Docker Desktop / Docker Engine 的网络规则使用可达的宿主机地址。

如果通过普通 HTTP 内网地址测试登录，例如 `http://192.168.31.25:3200`，需要在 `docker/.env` 中设置：

```env
WEB_PORT=3200
API_ALLOWED_ORIGINS=http://localhost:3200,http://127.0.0.1:3200,http://192.168.31.25:3200
API_COOKIE_SECURE=false
```

正式生产环境使用 HTTPS 时应保持 `API_COOKIE_SECURE=true`。否则浏览器会拒绝在明文 HTTP 页面保存 `Secure` session cookie，表现为登录接口返回成功，但刷新 `/api/console/me` 时仍然是未登录。

镜像版本、端口、数据库、API、插件运行器和初始化 root 账号配置都集中在 `docker/.env.example`。不再需要复制 `api/api.env`、`plugin-runner/plugin-runner.env` 或 `postgres/postgres.env`。

官方插件默认要求官方签名校验，`API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true` 会拒绝未签名或无法用 trusted key 验证的官方 / 镜像源插件。自托管环境明确接受风险时可设为 `false`；此时仍校验 registry 中的 `sha256` checksum，未验证包会标记为 `unverified`。

默认服务：

- `web`: `http://127.0.0.1:3100`
- `api`: compose 内部 `http://api:7800`
- `plugin-runner`: compose 内部 `http://plugin-runner:7801`
- `db`: compose 内部 `db:5432`

默认镜像：

- `ghcr.io/taichuy/1flowbase-web:${FLOWBASE_WEB_VERSION}`
- `ghcr.io/taichuy/1flowbase-api-server:${FLOWBASE_API_SERVER_VERSION}`
- `ghcr.io/taichuy/1flowbase-plugin-runner:${FLOWBASE_PLUGIN_RUNNER_VERSION}`

默认使用每个组件镜像的 `latest` tag；生产部署或需要可复现回滚时，可以在 `.env` 里把单个组件 pin 到具体版本，例如 `FLOWBASE_WEB_VERSION=v0.1.1`。
官方镜像发布 `linux/amd64` 和 `linux/arm64` manifest，Docker 会按本机平台自动选择。部署脚本在拉取或启动前会检测有效 Docker 平台，并提前提示当前 tag 是否缺少对应 manifest；如果需要临时强制使用 x86 镜像，可以设置 `DOCKER_DEFAULT_PLATFORM=linux/amd64`。

`web` 镜像内置 nginx，用于托管前端静态文件并把 `/api`、`/health`、`/openapi.json` 反代到 `api:7800`。生产部署默认不挂载后端二进制和前端 `dist`，这些构建产物必须随镜像 tag 发布。

默认持久化和可编辑配置都在 `docker/` 下：

- `postgres/data/pgdata/`: PostgreSQL 数据
- `api/storage/`: API 本地文件存储
- `api/plugins/packages/`: 插件包缓存
- `api/plugins/installed/`: 插件安装产物
- `api/plugins/host-extension/dropins/`: HostExtension drop-in 目录
- `web/nginx.conf`: web 镜像的 nginx 覆盖配置

注意：不要把整个 `api/plugins` 挂载到容器里；镜像内置的 `api/plugins/host-extensions` 和 `api/plugins/sets` 是启动所需的官方插件工作区，只挂载上面的可写子目录。

## 本地构建缓存

Dockerfile 已启用 BuildKit cache mount：

- Rust 镜像缓存 cargo registry、git checkout 和 release `target` 中间产物。
- web 镜像缓存 pnpm store，并把依赖安装层和源码层拆开。

第一次本地构建仍然会慢；之后只改业务源码时，会复用依赖下载和大部分中间编译产物。

本地需要 Docker BuildKit/buildx 可用。先确认：

```bash
docker buildx version
```

从仓库根目录手动构建：

```bash
docker buildx build --load -f docker/api-server.Dockerfile -t ghcr.io/taichuy/1flowbase-api-server:local .
docker buildx build --load -f docker/plugin-runner.Dockerfile -t ghcr.io/taichuy/1flowbase-plugin-runner:local .
docker buildx build --load -f docker/web.Dockerfile -t ghcr.io/taichuy/1flowbase-web:local .
```

使用本地镜像启动：

```bash
cd docker
FLOWBASE_WEB_VERSION=local FLOWBASE_API_SERVER_VERSION=local FLOWBASE_PLUGIN_RUNNER_VERSION=local docker compose up -d
```

CI 发布镜像时也会继续使用 GitHub Actions cache，因此同一镜像的后续构建会复用远端缓存。CI 使用 buildx 发布 `linux/amd64` 和 `linux/arm64`。镜像按组件 manifest 版本自动发布：`web/app/package.json` 的 `version` 变化只发布 `1flowbase-web:vX.Y.Z` 并更新 `1flowbase-web:latest`，`api/apps/api-server/Cargo.toml` 的 `version` 变化只发布 `1flowbase-api-server:vX.Y.Z` 并更新 `1flowbase-api-server:latest`，`api/apps/plugin-runner/Cargo.toml` 的 `version` 变化只发布 `1flowbase-plugin-runner:vX.Y.Z` 并更新 `1flowbase-plugin-runner:latest`。普通源码提交不会发布镜像。
