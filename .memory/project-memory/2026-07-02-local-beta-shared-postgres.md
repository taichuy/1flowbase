---
created_at: "2026-07-02 13"
updated_at: "2026-07-02 13"
decision_policy: verify_before_decision
topic: "local beta database container"
---

# Local beta database container

本地 `1flowbase_latest` / beta 工作区应复用 `../1flowbase` 的 Postgres 容器和 `35432` 端口，但使用独立 database `1flowbase_latest`；`../1flowbase` 继续使用同一容器内的 `1flowbase` database。

这样做是因为两个 git 工作区只是前后端端口不同，不需要拆出第二个 Postgres 容器；database 仍然隔离，避免 beta/latest 与主工作区数据互相污染。

后续调整本地 dev-up 或 Docker 中间件时，先核对 latest 是否仍指向 `127.0.0.1:35432/1flowbase_latest`，中间件 Compose 是否仍解析到 project `docker` 和主工作区 PGDATA。
