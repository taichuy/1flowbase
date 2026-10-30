# 后端本地一次性命令

[English](README.en.md)

`POST /api/console/local-commands/execute` 接收一个命令字符串，在接收请求的后端环境中执行，等待结束后返回普通 JSON 响应：

```json
{"command":"pwd && ls -la"}
```

## 权限与部署

使用现有 1flowbase 认证。Cookie 请求需要现有 `x-csrf-token`。在角色的「其他权限 → Local commands」中选择执行接口即可授权；取消选择或关闭权限组后拒绝执行。`full/custom` 沿用既有规则，关闭组不会清空自定义选择。root 沿用系统超级管理员规则，接口内部没有额外 root-only 判断。

无需启用配置或环境变量。旧的 `API_LOCAL_COMMAND_TIMEOUT_MS`、`API_LOCAL_COMMAND_MAX_OUTPUT_BYTES` 已删除，不再影响启动或执行。接口不设置独立的命令时长上限或输出总量上限，也不截断输出。没有调用期限时等待命令结束；一次性不表示任意命令必定快速结束。

命令使用后端服务的操作系统账号、工作目录和环境变量，不自动提权，不执行在浏览器所在电脑：

| 后端环境 | 命令解释器 | 示例 |
| --- | --- | --- |
| Linux / macOS 原生 | `/bin/sh -c` | `pwd && ls -la` |
| Windows 10+ / Server 2016+ 原生 | 系统 `cmd.exe /D /S /C` | `cd && dir` |
| Linux Docker 容器 | 容器内部的 `/bin/sh -c` | `pwd && ls -la` |

Windows/macOS 主机上的 Linux 容器仍执行 Linux 命令。命令所需程序必须安装在实际执行环境里。Windows 需要 PowerShell 时可在 command 字符串中显式调用 `powershell.exe`；接口不会把不同系统的命令语法互相翻译。

## 结果与输出

响应仍为 `{ "data": { ... }, "meta": null }`，data 包含：

| 字段 | 含义 |
| --- | --- |
| `stdout` / `stderr` | 两路文本输出，各自保持顺序，不提供两路全局时间顺序 |
| `exit_code` | shell 的退出码；非零也是有效结果；Unix 信号终止时可为 null |
| `signal` | Unix 终止信号编号，其他情况为 null |
| `duration_ms` | 命令执行、采集和回收耗时，不含 HTTP 发送 |
| `timed_out` | 当前接口没有专属超时，正常返回为 false |
| `truncated` / `discarded_bytes` | 当前不做预算截断，分别为 false / 0 |
| `encoding_lossy` | 存在无效 UTF-8，以替代字符表示 |
| `output_complete` | 两路输出完整采集后才返回 true；读取、写入失败返回错误 |
| `cleanup_complete` | Windows 确认 Job 无活动进程；Unix 确认进程组消失。未确认则为 false |

Windows 为每次调用创建私有隐藏控制台并将代码页设置为 UTF-8，不修改后端或用户终端的代码页。无视该代码页的旧程序可能仍输出非 UTF-8 字节，此时按同一规则报告 `encoding_lossy`；接口不猜测未知程序的编码。

两路输出边读边写到本次调用的临时文件。文件保存按块转义后的 JSON 字符串内容；命令结束后，HTTP 按块读取并返回，保持普通 JSON 契约，不使用 SSE、WebSocket 或会话 ID。内存只保留处理块及元数据，临时磁盘随实际输出增长；不把全部文件再次读入内存。Unix 文件创建后立即 unlink（权限 0600），Windows 使用 delete-on-close；响应结束或放弃发送时关闭句柄并回收，不提供历史查询，不进入 ephemeral。

磁盘满、进程启动或读取失败明确报错；不伪装为成功截断。响应发送阶段的读取或网络失败会使响应不完整，不能当作成功 JSON。非零退出码不等同于 HTTP 失败。无效命令为 400；未认证或缺少 CSRF header 为 401；权限不足或 CSRF 不匹配为 403；执行设施失败为 500。

## 生命周期与证据

每次执行新 shell，stdin 为 EOF，`cd` 和 shell 变量不跨调用共享。Unix 使用独立进程组；Windows 使用 Job Object，在创建进程时即纳管，并只继承本次标准输入输出句柄，避免并发调用互相持有管道。shell 结束后终止其受管后台进程，再收集尾部输出。Windows 由 Job 回收其受管进程；Unix 中尚未被系统回收的后代也可能使 cleanup_complete 暂时为 false。

HTTP 断连不会主动取消业务，完整调用由独立任务持有；内部执行 Future 被取消时触发本次进程清理。后端不重试，客户端自行重试可能重复产生文件等外部副作用。命令正文与输出不写入普通审计或插件观察投影，审计只保留调用者、执行标识和结果元数据。

这不是沙箱，不保证终止主动逃离管理边界的进程，也不回滚命令副作用。反向代理自身超时仍可能先于命令结束。

`.github/workflows/local-command.yml` 在 Windows、Linux、macOS 原生 runner 上直接编译并测试生产执行模块。Linux 的 API 测试另验证真实认证、角色授权、撤销与完整响应。轻量原生测试不等于整个原生发行包已经验证；每次交付以实际 Actions 结果和候选 SHA 为准。
