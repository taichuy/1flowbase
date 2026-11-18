# 本地 terminal

[English](README.en.md)

`terminal` 在 API 后端实例的操作系统账号下工作，操作的是后端文件系统。它不提供浏览器电脑访问、路径沙箱或 PTY。Unix 使用 `/bin/sh`；Windows 使用系统目录中的 `cmd.exe`，不接受客户端或 `COMSPEC` 指定 shell。旧页面与 `POST /api/console/local-commands/execute` 保持原契约。

全部新增操作属于已有 `other.local-commands` 授权组，每个接口独立授权。接口 ID 保留下划线，例如 `terminal.files.read_text`；HTTP 为 `POST /api/console/terminal/files/read-text`。`terminal.info` 对应 `POST /api/console/terminal/info`，请求 `{}`。MCP 工具目录由独立配置投影这些接口。

文件操作要求系统原生绝对路径和正数 `timeout_ms`。`list` 使用显式 `cursor`、`limit`，目录顺序不保证跨请求稳定；`stat` 返回元数据。`read_text` 要求 `offset`、`max_bytes`，只返回完整 UTF-8 字符；`read_range` 要求 `offset`、`length`，返回 base64。两者可传 `expected_version`；版本只检测弱元数据变化，不保证文件快照。偏移等于文件大小返回空 EOF，大于文件大小报错；新读取拒绝符号链接、管道和设备等特殊文件。

`search` 接受单行字面串 `pattern`，必须显式指定 `max_scan_bytes`、`max_matches`、`max_depth`、`max_line_bytes`。目录按惰性深度优先遍历；超长行、二进制与特殊文件分别计数，深度和预算截断可见。匹配项的 `byte_offset` 是该行起点，行号从 1 开始。

`read_image`、`read_audio` 要求 `max_file_bytes`，按文件签名识别 JPEG/PNG/GIF/WebP 和 WAV/MP3/Ogg/FLAC。`read_resource` 使用同样的预算；可选 `mime_type` 默认 `application/octet-stream`，只接受合法的 type/subtype，无参数。`link_only=true` 只读取元数据，返回 `data:null`。资源 URI 是编码后的合法 `file:` URL，不携带文件正文；MCP 宿主将其包装成受当前调用者权限约束的可重放引用。

`import_resource` 接受宿主资源引用 `resource_uri` 和目标 `file_table_id`，重放时重新验证来源及当前调用者，并通过目标文件表上传权限。`download_file` 接受 `file_table_id`、`record_id`、`target_path`、`timeout_ms`，使用文件访问服务检查来源 ACL。下载在同目录独占创建临时文件，校验完整来源流长度、SHA256 与临时文件重读结果后原子提交；默认拒绝覆盖，只有 `overwrite=true` 才替换。可选 `expected_sha256` 提供调用者已知的来源摘要断言。响应摘要证明本次传输完整性，不代表已验证不存在的存储原始摘要。失败会清理临时文件。

`jobs.start` 要求 `request_id`、`command`、`timeout_ms`、`retention_ms`、`max_output_bytes`。同一用户、租户、工作区中的相同 request ID 和参数幂等复用任务；不同参数冲突。`get/read/cancel` 仅允许该 owner，管理员不冒充其他任务 owner。`read` 指定 `stream`、`offset`、`max_bytes`；`cancel` 请求终止，终态与输出/清理完成分开报告。任务保存在当前进程唯一共享 registry 中，generation 更新复用，重启后不保留。

测试位于 `_tests/files.rs` 与 `routes/terminal/_tests`。纯文件行为由现有 native platform fixture 编译同一生产源码；HTTP、授权、真实文件表导入下载由宿主测试覆盖，统一由集中 QA 执行。
