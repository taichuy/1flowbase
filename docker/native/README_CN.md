# 1flowbase 原生便携版（试验）

无需安装 Docker、Nginx、Node.js 或 PostgreSQL。包内 Rust 后端同时提供网页和 API，并管理随包 PostgreSQL 18。第三方插件支持范围取决于其发布的平台产物。

## 启动

解压后，Windows 双击 `1flowbase.exe`；Linux/macOS 用普通用户执行 `./start.sh`，不要使用 sudo。启动后浏览器打开 `http://127.0.0.1:7800`；无图形环境时手动访问该地址。终端保持运行，按 Ctrl+C 正常退出。试验版不注册系统服务；请勿通过关闭窗口或强制结束进程代替正常退出。

默认数据库端口为 `35432`，仅监听 `127.0.0.1`。端口占用会报错，不连接、不停止其他数据库。Windows ARM64 等组合需要匹配的 PostgreSQL 和插件产物，不以包名代表平台验收。

首次初始化使用构建时 `docker/.env.example` 中的默认账号密码：账号 `root`，密码 `root@1flowbase123`，邮箱 `root@1flowbase.com`。初始密码也保存在程序旁 `data/settings.json` 的 `root_password` 字段。数据库密码和凭据加密密钥仍独立随机生成，不要分享整个配置文件。该默认值用于首次创建账号，重启不会重置已有账号密码。

## 数据与配置

三平台统一默认使用**程序所在目录下的 `data/`**，与从哪个工作目录启动无关。请解压到可写目录；不会自动回退到用户目录。

```text
1flowbase-windows-x64/
├── 1flowbase.exe
├── postgres/             # 随包数据库程序
└── data/                 # 首次启动生成，升级时保留
    ├── settings.json     # root_password、数据库密码、加密密钥
    ├── postgres/         # 数据库数据
    ├── postgres.log
    ├── api/              # 业务文件、已安装插件等
    └── backups/
```

Linux/macOS 的可执行文件为 `1flowbase`，其余数据布局相同。程序包不携带已有 `data/` 或用户密码。

| 环境变量 | 默认值 | 用途 |
| --- | --- | --- |
| `FLOWBASE_DATA_DIR` | `<程序目录>/data` | 显式指定其他持久数据目录；相对值按启动时的工作目录解析 |
| `FLOWBASE_PORT` | `7800` | 网页/API 本机端口 |
| `FLOWBASE_POSTGRES_PORT` | `35432` | 私有数据库端口 |
| `FLOWBASE_OPEN_BROWSER` | 启用 | 设置为 `false` 不自动打开浏览器 |

Linux/macOS 示例：`FLOWBASE_PORT=17800 FLOWBASE_POSTGRES_PORT=45432 ./start.sh`。Windows 可在 PowerShell 中设置 `$env:FLOWBASE_PORT='17800'` 后执行 `.\1flowbase.exe`。

同一数据目录只允许一个实例。不要删除 `settings.json` 或在数据库仍运行时复制/移动数据目录。程序升级需先正常退出并创建可恢复的备份，再解压新包；保留整个 `data/` 及其配置。若换到新的程序目录，将完整 `data/` 复制到新目录后再启动。试验版不自动执行 PostgreSQL 大版本迁移，不保证 schema 迁移后可直接回退旧程序。异常退出后如提示 PG 仍在运行，应先核实该实例、使用本包 `postgres/bin/pg_ctl -D <数据目录>/postgres stop -m fast -w` 正常停止，再重试；不要手动删除 PID 文件。

## 从早期试验版迁移

早期默认位置：Windows `%LOCALAPPDATA%\1flowbase`；Linux `${XDG_DATA_HOME:-$HOME/.local/share}/1flowbase`；macOS `~/Library/Application Support/1flowbase`。

如果新版还没有自己的配置/数据库，但发现旧目录中有配置或数据库，会停止并显示旧路径和新路径，不会新建空库或自动搬动数据。

1. 先用 Ctrl+C 正常停止旧程序，并确认其 PostgreSQL 已停止。
2. 将旧目录的**全部内容**复制到新程序目录的 `data/`，包括 `settings.json`、`postgres/`、业务文件和插件；不要只复制密码文件，也不要合并两个已有数据库。
3. 启动新版，确认原账号和数据可用后，再自行处理保留的旧目录备份。

若明确需要继续用旧目录，可用 `FLOWBASE_DATA_DIR` 指向它。新目录已有数据时直接使用新目录，不自动合并或覆盖旧目录。

## 构建与验证边界

每个平台分别构建 API 和 `system_recovery`，构建 Web 静态资源，并准备同平台 PostgreSQL 18 的完整可重定位发行目录（包含共享库、share、许可证以及 bin 下 postgres、pg_ctl、initdb、psql、createdb、pg_dump、pg_restore）。构建机可使用 Node.js；用户运行时不需要。

```sh
cargo build --manifest-path api/Cargo.toml --release -p api-server --bin api-server --bin system_recovery
pnpm --dir web build
node scripts/node/native-release/resources.js tmp/native-resources
node scripts/node/native-release/fetch-postgres.js linux-x64 tmp/native-postgres
# postgres-dir 使用上一步输出的 PostgreSQL 目录。
node scripts/node/native-release/package.js \
  --platform linux --arch x64 \
  --api-binary api/target/release/api-server \
  --recovery-binary api/target/release/system_recovery \
  --web-dir web/app/dist --postgres-dir /path/to/postgresql \
  --resources-dir tmp/native-resources --out tmp/native-release
```

系统名为 `linux`、`macos`、`windows`，架构名为 `x64`、`arm64`。包名例如 `1flowbase-windows-x64.zip`；版本写入 Release 和 `native-distribution.json`，文件名不含 hash。打包器拒绝覆盖已存在的目录或归档。

生产发行应通过 `--resources-dir` 携带现有 model-pricing / application-templates 资源，保留 PostgreSQL 及附带库的许可证，并在干净目标系统验证完整功能、共享库依赖、备份恢复、签名/公证。本试验不证明全部插件支持三平台，也不包含自动更新和系统服务。

Windows 网络出口插件目前只回收直接子进程，进程树退出收据会明确报告未验证；在 Windows Job Object 支持完成前，不将该能力用于需要保证无后代进程残留的部署。
