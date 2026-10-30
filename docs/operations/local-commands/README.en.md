# One-shot commands on the backend host

[中文](README.md)

`POST /api/console/local-commands/execute` runs one shell command in the backend instance receiving the request, waits for completion, and returns its output. The request has one field:

```json
{"command":"pwd && ls -la"}
```

The endpoint uses existing 1flowbase authentication. Cookie-authenticated requests require the existing `x-csrf-token`. Only the system root user can execute commands; granting an interface operation to an ordinary role does not grant OS access. The command uses the backend service's OS account, working directory, and environment. Application root does not imply OS root or elevation.

## Enable execution

Execution is disabled by default. Set both positive integers in the backend deployment environment and start the backend through the normal deployment process:

- `API_LOCAL_COMMAND_TIMEOUT_MS`: command execution deadline in milliseconds.
- `API_LOCAL_COMMAND_MAX_OUTPUT_BYTES`: shared raw output byte budget for stdout and stderr.

There are no built-in business capacity defaults. Configure values for your deployment. A missing counterpart, invalid value, or unsupported platform fails configuration validation. The current implementation supports Unix environments with `/bin/sh` and POSIX process groups. Windows is not implemented; it does not silently fall back to execution without process-group cleanup.

For example, a deployment needing up to 30 seconds and 1 MiB of raw output could use `30000` and `1048576`. These are examples, not defaults or fixed upper bounds. The output budget applies to retained raw bytes, not process RSS, JSON response size, or aggregate concurrent memory.

## Response

Successful HTTP responses use the existing `{ "data": { ... }, "meta": null }` envelope:

| Field | Meaning |
| --- | --- |
| `stdout` / `stderr` | Standard output and standard error, preserving order within each stream; no global ordering across streams |
| `exit_code` | Shell exit code; a nonzero code is a valid command result; may be null after a signal |
| `signal` | Unix termination signal number, or null for an ordinary exit |
| `duration_ms` | Execution and cleanup duration |
| `timed_out` | Whether the execution deadline was reached |
| `truncated` / `discarded_bytes` | Whether the output budget was exceeded and how many raw bytes were discarded |
| `encoding_lossy` | Invalid UTF-8 was replaced; a budget cutting a multibyte character can also cause this |
| `output_complete` | Both pipes reached EOF with no read/retention errors; does not imply absence of budget truncation |
| `cleanup_complete` | The direct child was reaped and the invocation's process group was confirmed absent |

Both streams are drained concurrently. Once the budget is exhausted, additional output is read and discarded to prevent pipe deadlock. Ordinary command failures appear in the exit code and output. Invalid command content returns 400, missing authentication or a missing CSRF header 401, non-root/mismatched CSRF/disabled execution 403, and process startup or infrastructure failures 500. The existing JSON extractor rejects invalid types and extra fields.

Command text and output are excluded from audit logs and ordinary managed-plugin observations. Audit records contain the actor, execution identifier, and terminal metadata. A terminal audit write failure is logged server-side without reporting an already-executed command as unexecuted.

## Lifecycle boundaries

Each request starts a fresh `/bin/sh -c` with stdin at EOF. Pipes, redirections, quoting, and compound commands are supported. There is no interactive terminal, SSH connection, session ID, or subsequent input. Shell variables and `cd` do not persist between calls; external effects such as file writes can persist.

At the execution deadline or when the shell exits, the backend sends SIGKILL to the invocation's process group, drains remaining output, and waits for cleanup. Cleanup may take one additional configured execution period, so HTTP waiting can exceed `API_LOCAL_COMMAND_TIMEOUT_MS`. Configure reverse-proxy timeouts to cover execution and cleanup. Unconfirmed cleanup returns `cleanup_complete=false`; incomplete pipe drainage returns `output_complete=false`. Sending a signal is not treated as proof of cleanup.

HTTP disconnection does not cancel the command. An independent owner continues until completion or the deadline. The backend never retries; a client retry after a lost response can duplicate side effects. There are no persistent jobs, output caches, or resumable sessions.

Local refers to execution location. A container deployment executes inside that container; a multi-instance deployment executes on the instance receiving the request. This is not a sandbox and does not prevent file or network access available to the OS account. Cleanup is not guaranteed for processes that detach from the group or after the backend is forcibly killed, and external effects are not rolled back.
