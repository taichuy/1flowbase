use super::*;

#[test]
fn local_command_requires_explicit_valid_deployment_budgets() {
    assert!(policy_from_env(&BTreeMap::new()).unwrap().is_none());
    let mut map = BTreeMap::from([("API_LOCAL_COMMAND_TIMEOUT_MS".into(), "1000".into())]);
    assert!(policy_from_env(&map).is_err());
    map.insert("API_LOCAL_COMMAND_MAX_OUTPUT_BYTES".into(), "0".into());
    assert!(policy_from_env(&map).is_err());
    map.insert("API_LOCAL_COMMAND_MAX_OUTPUT_BYTES".into(), "1024".into());
    #[cfg(unix)]
    assert_eq!(
        policy_from_env(&map).unwrap().unwrap().max_output_bytes,
        1024
    );
    map.insert("API_LOCAL_COMMAND_TIMEOUT_MS".into(), "-1".into());
    assert!(policy_from_env(&map).is_err());
}

#[cfg(unix)]
mod process {
    use super::*;
    fn policy(bytes: usize) -> LocalCommandPolicy {
        LocalCommandPolicy {
            timeout: Duration::from_secs(3),
            max_output_bytes: bytes,
        }
    }
    #[tokio::test]
    async fn local_command_shell_output_nonzero_and_stdin_eof() {
        let result = HostCommandExecutor
            .execute(
                "cat; printf hello | tr a-z A-Z; printf problem >&2; exit 7".into(),
                policy(1024),
            )
            .await
            .unwrap();
        assert_eq!(result.stdout, "HELLO");
        assert_eq!(result.stderr, "problem");
        assert_eq!(result.exit_code, Some(7));
        assert!(!result.timed_out);
        assert!(result.output_complete && result.cleanup_complete);
    }
    #[tokio::test]
    async fn local_command_drains_both_pipes_after_shared_budget_is_full() {
        let result = HostCommandExecutor
            .execute(
                "(head -c 262144 /dev/zero >&2) & head -c 262144 /dev/zero; wait".into(),
                policy(4096),
            )
            .await
            .unwrap();
        assert_eq!(result.stdout.len() + result.stderr.len(), 4096);
        assert_eq!(result.discarded_bytes, 524288 - 4096);
        assert!(result.truncated && result.output_complete && result.cleanup_complete);
        assert!(!result.timed_out);
    }
    #[tokio::test]
    async fn local_command_timeout_reaps_direct_child() {
        let result = HostCommandExecutor
            .execute(
                "exec sleep 30".into(),
                LocalCommandPolicy {
                    timeout: Duration::from_millis(100),
                    max_output_bytes: 1024,
                },
            )
            .await
            .unwrap();
        assert!(result.timed_out);
        assert_eq!(result.signal, Some(9));
        assert_eq!(result.exit_code, None);
        assert!(result.cleanup_complete);
    }
    #[tokio::test]
    async fn local_command_background_pipe_does_not_keep_request_alive() {
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            HostCommandExecutor.execute(
                "sleep 30 & printf ready".into(),
                LocalCommandPolicy {
                    timeout: Duration::from_millis(100),
                    max_output_bytes: 1024,
                },
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.stdout, "ready");
        assert_eq!(result.exit_code, Some(0));
        assert!(result.output_complete);
        assert!(!result.timed_out);
    }
    #[tokio::test]
    async fn local_command_does_not_retain_shell_state_and_marks_lossy_output() {
        let first = HostCommandExecutor
            .execute(
                "export ONEFLOWBASE_LOCAL_COMMAND_TEST=present; printf '\\377'".into(),
                policy(1024),
            )
            .await
            .unwrap();
        assert!(first.encoding_lossy);
        let second = HostCommandExecutor
            .execute(
                "printf %s \"${ONEFLOWBASE_LOCAL_COMMAND_TEST-unset}\"".into(),
                policy(1024),
            )
            .await
            .unwrap();
        assert_eq!(second.stdout, "unset");
    }
    #[tokio::test]
    async fn local_command_caller_drop_does_not_abandon_execution() {
        let path = std::env::temp_dir().join(format!("local-command-{}", uuid::Uuid::now_v7()));
        let command = format!("sleep 0.1; printf done > '{}'", path.display());
        let task =
            tokio::spawn(async move { HostCommandExecutor.execute(command, policy(1024)).await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(3), async {
            while !path.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"done");
        std::fs::remove_file(path).unwrap();
    }
}
