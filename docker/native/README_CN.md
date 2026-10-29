# 1flowbase 原生便携版

包内包含 Rust 后端、Web 静态资源、PostgreSQL 18 和维护入口。运行与升级不需要安装 Docker、Nginx、Node.js 或 Python。

## 首次启动与配置

从 GitHub Releases 下载对应系统/架构的压缩包，解压一次到可写目录。Windows 双击 `1flowbase.exe`；Linux/macOS 用普通用户运行 `./start.sh`。默认浏览器打开 `http://127.0.0.1:7800`。终端保持运行，Ctrl+C 正常退出，等待数据库停止。

正式包根目录的 `.env` 是实际运行配置，来源为 `api/apps/api-server/.env.example`，打包时适配原生生产模式、本机地址与资源路径。默认账号 `root`，初始密码 `root@1flowbase123`；可在首次启动前修改 `BOOTSTRAP_ROOT_*`。已创建账号不会因重启被重置。

```text
1flowbase-windows-x64/
├── 1flowbase.exe
├── .env                      # 用户实际配置，升级原样保留
├── .env.example              # 本版本默认配置，升级后对照新增项
├── upgrade.cmd               # Windows 升级/恢复入口
├── native-distribution.json  # 版本、平台、PG 大版本及文件校验清单
├── api/  web/  postgres/     # 受管程序与资源
└── data/                     # 首次启动生成，升级保留
    ├── postgres/             # 数据库数据
    ├── postgres.log
    ├── api/                  # 业务文件与已安装插件
    └── backups/
```

Linux/macOS 的主程序为 `1flowbase`，升级入口为 `upgrade.sh`，另带 `start.sh`。包名只表达系统/架构，版本和校验摘要记录在 Release/manifest 中。

配置读取规则：

- 始终读取可执行文件旁的 `.env`，不取决于终端工作目录；相对路径也以该目录为基准。
- `.env` 中声明的值覆盖同名进程环境变量；不再读取 `settings.json` 或编译内嵌 Docker 默认值。
- 文件为 UTF-8，支持 `KEY=value`、单引号字面量、JSON 风格双引号转义及注释；不执行 shell、不展开变量。重复 key 或格式错误会报错。
- 数据库密码和加密密钥的 `__GENERATE__` 占位符在首次启动时生成并写回 `.env`，后续保持稳定。必须一起保留 `.env` 和完整数据目录。已有数据库不会重新生成密钥。
- 配置修改后重启生效。数据库密码需与已有 PG 保持一致；加密密钥不能随意更换。

| 配置项 | 原生默认值 / 作用 |
| --- | --- |
| `API_SERVER_ADDR` | `127.0.0.1:7800`，网页/API 地址 |
| `API_DATABASE_URL` | 私有 PG 的唯一连接配置，端口默认 `35432` |
| `API_AUTH_PUBLIC_ORIGIN` / `API_ALLOWED_ORIGINS` | 默认对应 7800；修改访问地址时同步调整 |
| `FLOWBASE_DATA_DIR` | `data`，数据库及实例数据目录 |
| `API_BUSINESS_FILE_LOCAL_ROOT` 等存储路径 | 默认位于 `data/`；改数据位置时同步调整相关路径 |
| `FLOWBASE_OPEN_BROWSER` | `true`；改为 `false` 禁止自动打开浏览器 |

私有数据库固定为本机 `postgres` 用户和 `1flowbase` 数据库。本发行入口不接管外部数据库，不停止其他实例；端口冲突会报错。

## 离线升级

此布局是正式升级基线；早期缺少根目录 `.env` 或格式清单的试验包不支持直接升级，本版不添加试验期密码/目录迁移。

1. 在旧程序窗口按 Ctrl+C，等待应用与 PG 正常停止。
2. 将新包解压到**旧安装目录以外**，不要直接覆盖旧目录。
3. 从新包运行升级入口，选择旧安装目录。Windows 双击 `upgrade.cmd` 会提示输入目录，也可执行：

```powershell
.\upgrade.cmd "E:\Apps\1flowbase"
```

Linux/macOS：

```sh
./upgrade.sh /path/to/existing/1flowbase
```

升级器校验文件清单、系统/架构、版本、PG 大版本、有效配置、空间和停机状态；创建完整冷备份；替换清单管理的程序文件；保留用户 `.env` 和数据；运行新程序的初始化、数据库迁移、健康检查和退出验证。验证时不开放业务 HTTP 服务、不打开浏览器。成功后从**原安装目录**正常启动程序。

备份位于旧安装目录的同级 `.<目录名>-upgrade-backups/<备份ID>/`，包含旧程序、`.env`、完整数据、校验收据和验证日志。升级不会自动删除备份。`.env.example` 更新为新版本模板；用户 `.env` 不被合并或覆盖。缺失必需配置时先停止，按模板补充后重试。

仅支持相同系统、架构、PostgreSQL 大版本的升级。PG 跨大版本需要单独迁移。数据必须在 `data/` 或独立外部目录，业务文件、插件和备份路径均需位于配置的数据目录内；符号链接/外部表空间不属于此冷备份支持范围，检测到时会拒绝操作。

## 升级失败与恢复

替换前校验/备份失败不会改变旧程序或数据。开始替换后，`.upgrade-state.json` 记录备份位置和阶段；失败或中断会阻止普通启动，保留恢复所需信息。

确认所有相关进程和 PG 已停止后，从**新包**运行明确的恢复命令：

```powershell
.\upgrade.cmd --restore "E:\Apps\1flowbase" "E:\Apps\.1flowbase-upgrade-backups\备份ID"
```

```sh
./upgrade.sh --restore /path/to/existing/1flowbase /path/to/.1flowbase-upgrade-backups/backup-id
```

恢复会验证备份归属及完整性，然后恢复同一次快照中的**程序、配置和完整数据**。恢复被中断可以重复执行同一命令。它会撤销快照之后的数据变更；不要仅替换旧 EXE 来回退已迁移的数据库。备份损坏、目标不匹配或进程仍运行时不会开始恢复。

启动验证日志在备份目录的 `startup-validation.log`。不要手工删除 PG 的 `postmaster.pid`；若异常退出留下运行中的 PG，应核实实例后使用本包 `postgres/bin/pg_ctl -D <数据目录>/postgres stop -m fast -w` 正常停止。

## 构建与验证边界

`Native portable distribution` 工作流构建全部原生产物，运行配置/升级正反例、真实 PG 生命周期和真实包升级恢复测试。填写 `release_tag` 并选择 `all` 可将单层归档放入 GitHub Releases；只有全部平台通过后才进入发布步骤。

`system_recovery` 继续用于系统备份恢复，不替代此程序升级入口。插件平台产物、签名/公证、系统服务和在线自动更新不由此离线升级功能承诺。Windows 网络出口插件的后代进程清理仍以其已有能力边界为准。
