# PostgreSQL Backup Toolchain

这个目录负责为本地源码开发提供可复现的 PostgreSQL 备份客户端，不属于 API Server 的运行时安装逻辑。

解析顺序固定为：

1. 同时配置的 `API_POSTGRES_PG_DUMP_PATH` 与 `API_POSTGRES_PG_RESTORE_PATH`；
2. `tmp/toolchains/postgresql/<version>/<target>/` 中收据和二进制均验证通过的缓存；
3. 当前系统 `PATH` 中版本兼容的 `pg_dump` 与 `pg_restore`；
4. `lock.json` 中当前平台对应的固定 artifact。

`dev-up` 会自动调用 resolver。也可以单独执行：

```bash
node scripts/node/postgres-toolchain/cli.js
```

下载器只接受清单内的平台、URL 与 SHA-256，先写临时文件，校验 archive 路径、工具版本与二进制摘要后，再原子安装并写入 `receipt.json`。下载、校验或平台识别失败只会禁用备份与还原能力，不会阻止 API 启动。

固定下载源是 `theseus-rs/postgresql-binaries` 的 PostgreSQL 18.4.0 release。它是第三方跨平台构建，采用 PostgreSQL License；下载产物只进入 Git 忽略的本地 `tmp/` 缓存，不提交仓库，也不进入正式镜像。

正式 API 镜像通过 PostgreSQL Global Development Group APT 仓库在构建期安装 `postgresql-client-18`。生产容器运行时不调用本目录代码，也不联网下载工具。
