# One-shot commands on the backend host

[中文](README.md)

`POST /api/console/local-commands/execute` accepts one command string, executes it in the backend instance receiving the request, waits for completion, and returns ordinary JSON:

```json
{"command":"pwd && ls -la"}
```

## Permissions and deployment

Use existing 1flowbase authentication. Cookie requests require the existing `x-csrf-token`. Grant the execution operation under the role's **Other permissions → Local commands** group. Removing that grant or disabling the group denies execution. Existing `full/custom` behavior applies; disabling a group preserves its custom selections. Root keeps its normal system-administrator access; the service has no additional root-only check.

No opt-in configuration or environment variables are required. `API_LOCAL_COMMAND_TIMEOUT_MS` and `API_LOCAL_COMMAND_MAX_OUTPUT_BYTES` have been removed and no longer affect startup or execution. This endpoint imposes no separate command duration or total output limit, and does not truncate output. Without a call deadline it waits until the command ends. One-shot does not mean that an arbitrary command must finish quickly.

Commands use the backend service's OS account, working directory, and environment, without elevation. They do not execute on the browser's computer:

| Backend environment | Interpreter | Example |
| --- | --- | --- |
| Native Linux / macOS | `/bin/sh -c` | `pwd && ls -la` |
| Native Windows 10+ / Server 2016+ | System `cmd.exe /D /S /C` | `cd && dir` |
| Linux Docker container | `/bin/sh -c` inside the container | `pwd && ls -la` |

A Linux container on a Windows/macOS host still executes Linux commands. Required programs must be installed in the execution environment. On Windows, invoke `powershell.exe` explicitly in the command string if needed. The endpoint does not translate shell syntax between platforms.

## Results and output

The response remains `{ "data": { ... }, "meta": null }`. Data fields:

| Field | Meaning |
| --- | --- |
| `stdout` / `stderr` | Text output, ordered within each stream; no total ordering between streams |
| `exit_code` | Shell exit code; nonzero is a valid command result; may be null after a Unix signal |
| `signal` | Unix termination signal, otherwise null |
| `duration_ms` | Execution, collection, and cleanup duration, excluding HTTP delivery |
| `timed_out` | No endpoint-specific timeout is imposed; false in normal returned results |
| `truncated` / `discarded_bytes` | No quota truncation; false / 0 |
| `encoding_lossy` | Invalid UTF-8 was replaced with replacement characters |
| `output_complete` | True only after both streams were collected; read/write failures return errors |
| `cleanup_complete` | Job has no active processes on Windows; process group is absent on Unix. False when not confirmed |

Each Windows invocation uses a private hidden console with its code page set to UTF-8, leaving the backend and user's console code pages unchanged. Legacy programs that ignore that code page may still emit non-UTF-8 bytes; the same `encoding_lossy` rule applies. Unknown application encodings are not guessed.

Both streams are drained into invocation-local temporary files containing incrementally escaped JSON string contents. After command completion, HTTP reads those files in chunks. This remains a normal JSON response, without SSE, WebSocket, or a session ID. Memory retains processing chunks and metadata; temporary disk use grows with actual output. Files are never read back into one large string. Unix files are immediately unlinked after creation with mode 0600; Windows uses delete-on-close. Closing response handles on completion or abandoned delivery releases the files. There is no history query or ephemeral-cache entry.

Disk-full, startup, and read errors are reported, not disguised as successful truncation. Read/network failure during response delivery leaves an incomplete response, which is not a successful JSON result. Nonzero exit codes are not HTTP failures. Invalid command content returns 400; missing authentication or CSRF header 401; insufficient permission or mismatched CSRF 403; execution infrastructure failures 500.

## Lifecycle and evidence

Every call starts a fresh shell with stdin at EOF. Shell variables and `cd` do not persist. Unix uses a dedicated process group. Windows creates the process inside a Job Object and inherits only this invocation's stdio handles, avoiding cross-call pipe inheritance. When the shell exits, its managed background processes are terminated and trailing output collected. Windows reclaims managed processes through the Job; unreaped Unix descendants can temporarily prevent cleanup_complete from being true.

HTTP disconnection does not cancel business execution: an independent task owns the complete invocation. Cancellation of the internal execution future initiates cleanup. The backend does not retry; client retries can duplicate external effects such as file writes. Ordinary audit/plugin observations exclude command text and output, retaining actor, execution identifier, and result metadata only.

This is not a sandbox, does not promise cleanup of processes escaping the management boundary, and does not roll back external effects. A reverse proxy may time out before a command finishes.

`.github/workflows/local-command.yml` compiles and tests the actual production execution module on native Windows, Linux, and macOS runners. Linux API tests separately exercise real authentication, role grants/revocation, and complete responses. The lightweight native tests are not validation of the entire packaged application; use the actual Actions result and candidate SHA for each delivery.
