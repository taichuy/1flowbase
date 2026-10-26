# 1flowbase 原生便携版（试验）

无需安装 Docker、Nginx、Node.js 或 PostgreSQL。包内 Rust 后端同时提供网页和 API，并管理随包 PostgreSQL 18。第三方插件支持范围取决于其发布的平台产物。

## 启动

解压后，Windows 双击 `1flowbase.exe`；Linux/macOS 用普通用户执行 `./start.sh`，不要使用 sudo。启动后浏览器打开 `http://127.0.0.1:7800`；无图形环境时手动访问该地址。终端保持运行，按 Ctrl+C 正常退出。试验版不注册系统服务；请勿通过关闭窗口或强制结束进程代替正常退出。

默认数据库端口为 `35432`，仅监听 `127.0.0.1`。端口占用会报错，不连接、不停止其他数据库。Windows ARM64 等组合需要匹配的 PostgreSQL 和插件产物，不以包名代表平台验收。

首次启动的 `root` 密码保存于数据目录 `settings.json` 的 `root_password` 字段；启动终端会显示文件位置。不要分享该文件，其中也包含数据库密码和凭据加密密钥。

## 数据与配置

默认持久目录：Linux `${XDG_DATA_HOME:-$HOME/.local/share}/1flowbase`；macOS `~/Library/Application Support/1flowbase`；Windows `%LOCALAPPDATA%\1flowbase`。

| 环境变量 | 默认值 | 用途 |
| --- | --- | --- |
| `FLOWBASE_DATA_DIR` | 上述用户目录 | 独立持久数据目录 |
| `FLOWBASE_PORT` | `7800` | 网页/API 本机端口 |
| `FLOWBASE_POSTGRES_PORT` | `35432` | 私有数据库端口 |
| `FLOWBASE_OPEN_BROWSER` | 启用 | 设置为 `false` 不自动打开浏览器 |

Linux/macOS 示例：`FLOWBASE_PORT=17800 FLOWBASE_POSTGRES_PORT=45432 ./start.sh`。Windows 可在 PowerShell 中设置 `$env:FLOWBASE_PORT='17800'` 后执行 `.\1flowbase.exe`。

同一数据目录只允许一个实例。不要删除 `settings.json` 或在数据库仍运行时复制/移动数据目录。程序升级需先正常退出并创建可恢复的备份，再解压新包；保留整个数据目录及其配置。试验版不自动执行 PostgreSQL 大版本迁移，不保证 schema 迁移后可直接回退旧程序。异常退出后如提示 PG 仍在运行，应先核实该实例、使用本包 `postgres/bin/pg_ctl -D <数据目录>/postgres stop -m fast -w` 正常停止，再重试；不要手动删除 PID 文件。

## 构建与验证边界

每个平台分别构建 API 和 `system_recovery`，构建 Web 静态资源，并准备同平台 PostgreSQL 18 的完整可重定位发行目录（包含共享库、share、许可证以及 bin 下 postgres、pg_ctl、initdb、psql、createdb、pg_dump、pg_restore）。构建机可使用 Node.js；用户运行时不需要。

```sh
cargo build --manifest-path api/Cargo.toml --release -p api-server --bin api-server --bin system_recovery
pnpm --dir web build
node scripts/node/native-release/fetch-postgres.js linux-x64 tmp/native-postgres
# postgres-dir 使用上一步输出的 PostgreSQL 目录。
node scripts/node/native-release/package.js \
  --platform linux --arch x64 \
  --api-binary api/target/release/api-server \
  --recovery-binary api/target/release/system_recovery \
  --web-dir web/app/dist --postgres-dir /path/to/postgresql \
  --out tmp/native-release
```

系统名为 `linux`、`macos`、`windows`，架构名为 `x64`、`arm64`。包名例如 `1flowbase-windows-x64.zip`；版本写入 Release 和 `native-distribution.json`，文件名不含 hash。打包器拒绝覆盖已存在的目录或归档。

生产发行应通过 `--resources-dir` 携带现有 model-pricing / application-templates 资源，保留 PostgreSQL 及附带库的许可证，并在干净目标系统验证完整功能、共享库依赖、备份恢复、签名/公证。本试验不证明全部插件支持三平台，也不包含自动更新和系统服务。
