# Local terminal

[简体中文](README.md)

Terminal operations run as the API backend's OS account against its native filesystem. They do not provide browser-machine access, a path sandbox, or a PTY. Unix uses `/bin/sh`; Windows uses `cmd.exe` from the system directory, ignoring client shell selection and `COMSPEC`. The existing page and `POST /api/console/local-commands/execute` retain their contract.

Every operation has its own authorization entry in the existing `other.local-commands` group. IDs retain underscores, such as `terminal.files.read_text`; HTTP uses `POST /api/console/terminal/files/read-text`. `terminal.info` uses `POST /api/console/terminal/info` with `{}`. MCP tools are configured separately as projections of these interfaces.

File operations require an absolute native path and positive `timeout_ms`. `list` requires `cursor` and `limit`; directory ordering is not stable across calls. `stat` returns metadata. `read_text` requires `offset` and `max_bytes` and returns complete UTF-8 characters. `read_range` requires `offset` and `length` and returns base64. Both accept `expected_version`, a weak metadata change detector rather than a snapshot guarantee. Offset equal to file size returns empty EOF; greater offsets fail. Reads reject symlinks and special files including pipes and devices.

`search` takes a single-line literal `pattern` and explicit `max_scan_bytes`, `max_matches`, `max_depth`, and `max_line_bytes`. Lazy depth-first traversal reports budget/depth truncation and separate long-line, binary-file, and special-file counts. Match `byte_offset` refers to the start of the line; line numbers start at one.

`read_image` and `read_audio` require `max_file_bytes` and identify JPEG/PNG/GIF/WebP and WAV/MP3/Ogg/FLAC by signature. `read_resource` uses the same budget and an optional `mime_type`, defaulting to `application/octet-stream`; it accepts a valid type/subtype without parameters. With `link_only=true`, it only reads metadata and returns `data:null`. Its encoded `file:` URL contains no file contents. The MCP host wraps it into a replayable reference subject to the current caller's authorization.

`import_resource` takes a host resource reference and `file_table_id`, rechecks source and caller authorization during replay, and uses destination file-table upload permissions. `download_file` takes `file_table_id`, `record_id`, `target_path`, and `timeout_ms`; the file access service enforces source ACLs. It exclusively creates a temporary file in the destination directory, verifies the complete stream length and SHA256 against a second read of staged bytes, then publishes atomically. Existing targets fail unless `overwrite=true`. Optional `expected_sha256` asserts a caller-known source digest. The response digest proves transfer integrity, not authenticity against an absent stored source digest. Failure cleans up staging files.

`jobs.start` requires `request_id`, `command`, `timeout_ms`, `retention_ms`, and `max_output_bytes`. The same request ID and parameters reuse the job within its user/tenant/workspace owner; changed parameters conflict. `get/read/cancel` require that owner, including for administrators. `read` specifies `stream`, `offset`, and `max_bytes`. Cancellation is a request; terminal state, output completion, and cleanup completion are reported separately. One process-local registry survives generation updates but not backend restart.

Tests live in `_tests/files.rs` and `routes/terminal/_tests`. The existing native platform fixture compiles the exact pure file implementation. Host tests cover HTTP, authorization, and real file-table transfers; execution belongs to centralized QA.
