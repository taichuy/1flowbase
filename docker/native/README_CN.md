# 1flowbase 原生便携版

包内包含 Rust 后端、Web 静态资源、PostgreSQL 18 和升级入口。运行与升级不需要安装 Docker、Nginx、Node.js 或 Python。

## 启动与配置

从 GitHub Releases 下载对应系统/架构的包，解压一次到可写目录。Windows 双击 `1flowbase.exe`；Linux/macOS 用普通用户运行 `./start.sh`。默认浏览器打开 `http://127.0.0.1:7800`。终端保持运行，Ctrl+C 正常退出，等待数据库停止。

根目录 `.env` 是实际运行配置，来源为 `api/apps/api-server/.env.example`，打包时适配原生端口、生产模式与资源路径。默认账号 `root`，初始密码 `root@1flowbase123`；可在首次启动前修改 `BOOTSTRAP_ROOT_*`。已创建账号不会因重启被重置。

```text
1flowbase-windows-x64/
├── 1flowbase.exe
├── .env                      # 用户配置，升级保留
├── .env.example              # 本版本默认配置
├── upgrade.cmd               # 双击升级，无需输入安装目录
├── upgrade.ps1               # Windows 升级启动脚本
├── native-distribution.json  # 版本、平台及文件清单
├── api/  web/  postgres/     # 程序与资源
└── data/                     # 数据库、业务文件和用户插件
```

Linux/macOS 使用 `1flowbase`、`start.sh` 和 `upgrade.sh`。包名只含产品、系统与架构，版本属于 Release。

- 始终读取可执行文件旁的 `.env`；相对路径以程序目录为基准，文件配置覆盖同名环境变量。
- 文件为 UTF-8，支持 `KEY=value`、单引号字面量、JSON 风格双引号转义及注释。不执行 shell，不展开变量；重复 key 或格式错误会报错。
- `__GENERATE__` 数据库密码和加密密钥在首次启动时生成并写回 `.env`，重启不变。必须保留原 `.env` 和数据，不要随意更换已有数据库密码或加密密钥。
- `API_SERVER_ADDR` 默认 `127.0.0.1:7800`；`API_DATABASE_URL` 指向随包 PG，默认 `127.0.0.1:35432`。私有数据库为 `postgres` 用户和 `1flowbase` 数据库。
- 改访问地址时同步修改 `API_AUTH_PUBLIC_ORIGIN` / `API_ALLOWED_ORIGINS`；改数据目录 `FLOWBASE_DATA_DIR` 时同步调整相关存储路径。
- `FLOWBASE_OPEN_BROWSER=false` 可关闭自动打开浏览器。配置修改后重启生效。

## 一键升级

1. 先按 Ctrl+C 退出应用，等待 PostgreSQL 正常停止。
2. 在**当前安装目录**双击 `upgrade.cmd`；Linux/macOS 运行 `./upgrade.sh`。
3. 等待下载、校验和更新完成，再正常启动程序。

不用手动选择系统、架构或输入安装目录，也不用预先下载新包。升级脚本按自身位置定位安装；Windows 使用系统自带 PowerShell，并在临时目录运行升级程序，避免正在运行的 EXE 无法替换。

升级器查询官方 `taichuy/1flowbase` GitHub Releases，选择有对应原生资产的更高版本：稳定版只接收稳定版，预发布版可接收更高的预发布版或稳定版。当前已是最新版本时提示并退出。网络连接失败时报告错误，不开始替换。

下载后校验官方资产的 SHA256、压缩包路径与文件清单，再检查平台、架构、PG 大版本、空间、配置及停机状态。随后**直接更新当前安装的程序文件，保留 `.env` 和用户数据，不创建备份，不提供回滚**。升级会执行新版本的初始化、迁移和健康检查；`.env.example` 更新为新版参考模板，用户 `.env` 不自动合并。

新包暂存在安装目录同级的 `.<安装目录名>-update/`，它只包含下载的新程序及更新日志，不复制旧程序、配置或数据库。成功后自动清理。

## 更新失败或中断

下载或校验失败不会替换当前程序。替换开始后，`.upgrade-state.json` 记录未完成状态，普通启动被阻止。确认应用和 PG 已停止后，**再次运行同一升级脚本**：有更新的修正版时下载修正版，否则继续安装已校验的新包。临时新包和状态文件不要手工删除。

初始化失败的日志位于 `.<安装目录名>-update/startup-validation.log`。按错误修正配置后可重试；如果是版本本身的问题，需要更新的修正版。由于没有备份，已完成的数据库迁移或数据修改不会回退。

仅支持同系统、架构、PostgreSQL 大版本的更新。PG 跨大版本需单独迁移。端口被占用、包损坏或程序仍运行时会停止，不接管其他数据库进程。

此一键在线入口从本版提供，旧版升级脚本不会自动获得新功能；初次使用本版需下载新包。早期缺少根 `.env` 的试验包不属于升级基线。

## 构建与验证边界

正式发版跟随 `container images`：使用 `node scripts/node/cli/bump-version.js` 同步版本并按现有流程合入 `latest` 后，自动构建 Docker 镜像和五个平台原生包。`VERSION`、前端和后端版本必须一致；两种发行的构建/验证都通过后，发布同一个正式 GitHub Release，原生包版本与 Docker 相同。

`native-build.yml` 是共用构建流程，继续验证实际归档、初始化、可编辑配置、直接更新不备份、失败重试及路径定位。`Native portable distribution` 保留手动验证/独立预发布入口；它不替代正式 Docker 发版流程。普通不改版本的提交不会自动生成新发行。已经完整发布且源码一致的 Release 重跑会跳过，不覆盖公开资产。

`system_recovery` 是独立的业务备份恢复工具，一键升级不会调用它。签名/公证、系统服务和静默后台自动更新不在本发行功能范围。Windows 网络出口插件的后代进程清理保留现有能力边界。
