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
