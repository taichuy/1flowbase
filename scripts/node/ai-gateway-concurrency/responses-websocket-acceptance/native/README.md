# Native Responses acceptance

Root #2028 的集中 Test Batch fixture；不能用脚本存在替代运行结果。

在 tmux 中执行 `node tool-chain.cjs PORT CLIENT_COUNT LABEL MODE`。环境：
`TEST_KEY_FILE` 指向权限600临时应用 key；`EVIDENCE_DIR` 指向本轮 `tmp/test-governance/issue-2028/current/records`；`ONEFLOW_REPO` 指向拥有 web/node_modules 的主仓。

- `7801 1 candidate normal`：候选真实三轮依赖工具。
- `7801 2 concurrent normal`：两个真实 Codex，两个网关 socket 均已建立后 barrier 才释放请求；同时核对随机 response/call identity。
- `7801 1 reconnect disconnect`：第一次工具已执行，回传前断开 socket；有界客户端恢复，不重复已完成工具。
- `7801 1 worker-restart worker-restart`：仅候选；必须提供 `CANDIDATE_WORKER_PID` 与其 `CANDIDATE_SERVER_PID`；核对父子身份后终止该 worker，观察真实错误及客户端恢复。禁止针对常用服务或无关进程。
- `7800 1 regular-after-restart normal`：完成正式配套启用和常用服务重启后全新会话；单列 regular 证据。

每次180秒上限，无普通模式重试。生成随机只读链，逐轮至少一次实际工具回传，最后回答严格匹配；证据保留结构、hash与关联ID，退出回收临时工作目录/子进程/relay。凭证由调用者负责最终回收。失败就检查本次证据，不循环重试。

现有 `../lifecycle.js`、`../error-matrix.js` 的真实网关场景独立覆盖断线/错误持久事实；定向 Rust bridge fixture 覆盖 writer failure/abort。任一没有执行的行仍是未验证。

本轮用户后续指示以1flowbase可用为重点；直连sub2api对照（AC-017）不阻塞本轮，标注未执行，不能记通过。
