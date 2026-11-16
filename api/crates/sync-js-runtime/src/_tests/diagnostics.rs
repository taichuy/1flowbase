use super::*;
use serde_json::json;

fn failure(source: &str) -> ExecutionError {
    run_legacy_script_cancellable(
        source,
        &json!({}),
        Limits {
            timeout: Duration::from_secs(1),
            memory_limit_bytes: 8 * 1024 * 1024,
            stack_size_bytes: 256 * 1024,
            max_output_bytes: 128,
        },
        &[],
        CancellationToken::default(),
    )
    .unwrap_err()
}

#[test]
fn legacy_main_error_keeps_bounded_diagnostic_without_stack() {
    let error = failure(
        "function main(){throw Error('Create deletion job HTTP 409: agent_logs_delete_job_active')}",
    );
    assert_eq!(error.code(), "runtime_error");
    assert_eq!(
        error.exception_message.as_deref(),
        Some("Create deletion job HTTP 409: agent_logs_delete_job_active")
    );
    assert_eq!(error.to_string(), "runtime_error: code execution failed");
    let error = failure("function main(){throw '应用清理失败'}");
    assert_eq!(error.exception_message.as_deref(), Some("应用清理失败"));
    let error = failure("function main(){throw Error('汉'.repeat(1000))}");
    let message = error.exception_message.unwrap();
    assert!(message.len() <= 128);
    assert!(message.chars().all(|ch| ch == '汉'));
}

#[test]
fn legacy_error_diagnostic_never_executes_accessors_proxies_or_coercion() {
    for source in [
        "function main(){const e=Error();Object.defineProperty(e,'message',{get(){console.error('getter ran');return 'unsafe'}});throw e}",
        "function main(){throw new Proxy(Error('unsafe'),{getOwnPropertyDescriptor(){console.error('proxy ran');return {value:'unsafe'}}})}",
        "function main(){throw {toString(){console.error('coercion ran');return 'unsafe'}}}",
        "Object.getOwnPropertyDescriptor=()=>{console.error('replacement ran');return {value:'unsafe'}};function main(){throw Error('safe')}",
    ] {
        let error = failure(source);
        assert!(error.console_logs.is_empty());
        assert!(error.exception_message.is_none() || error.exception_message.as_deref() == Some("safe"));
    }
}

#[test]
fn strict_plugin_errors_keep_the_existing_generic_contract() {
    let error = run_script(
        "function main(){throw Error('plugin private')}",
        &json!({}),
        Limits {
            timeout: Duration::from_secs(1),
            memory_limit_bytes: 8 * 1024 * 1024,
            stack_size_bytes: 256 * 1024,
            max_output_bytes: 128,
        },
    )
    .unwrap_err();
    assert!(error.exception_message.is_none());
    assert_eq!(error.to_string(), "runtime_error: code execution failed");
}
