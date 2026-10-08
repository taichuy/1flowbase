use super::*;

pub(super) fn compile_code_isolation_profile(
    node_id: &str,
    config: &Value,
    compile_issues: &mut Vec<CompileIssue>,
) -> CodeIsolationProfile {
    let mut profile = CodeIsolationProfile::quickjs_default();
    let Some(isolation) = config.get("isolation") else {
        return profile;
    };
    let Some(isolation) = isolation.as_object() else {
        push_code_isolation_issue(
            compile_issues,
            node_id,
            "isolation",
            "isolation must be an object",
        );
        return profile;
    };

    if let Some(value) = isolation_string(isolation.get("mode")) {
        match value {
            Ok(value) if value == CodeIsolationProfile::DEFAULT_MODE => {
                profile.mode = value.to_string();
            }
            Ok(_) => {
                push_code_isolation_issue(
                    compile_issues,
                    node_id,
                    "mode",
                    "only vm_limited code isolation mode is supported",
                );
            }
            Err(reason) => push_code_isolation_issue(compile_issues, node_id, "mode", reason),
        }
    }

    if let Some(value) = isolation_string(isolation.get("executor_id")) {
        match value {
            Ok(value) if value == CodeIsolationProfile::DEFAULT_EXECUTOR_ID => {
                profile.executor_id = value.to_string();
            }
            Ok(_) => {
                push_code_isolation_issue(
                    compile_issues,
                    node_id,
                    "executor_id",
                    "only quickjs-local code executor is supported",
                );
            }
            Err(reason) => {
                push_code_isolation_issue(compile_issues, node_id, "executor_id", reason);
            }
        }
    }

    if let Some(value) = bounded_u64(
        isolation.get("timeout_ms"),
        CodeExecutorCapability::QUICKJS_MAX_TIMEOUT_MS,
    ) {
        match value {
            Ok(value) => profile.timeout_ms = value,
            Err(reason) => push_code_isolation_issue(compile_issues, node_id, "timeout_ms", reason),
        }
    }

    if let Some(value) = bounded_u32(
        isolation.get("memory_mb"),
        CodeExecutorCapability::QUICKJS_MAX_MEMORY_MB,
    ) {
        match value {
            Ok(value) => profile.memory_mb = value,
            Err(reason) => push_code_isolation_issue(compile_issues, node_id, "memory_mb", reason),
        }
    }

    if let Some(value) = bounded_u32(
        isolation.get("stack_kb"),
        CodeExecutorCapability::QUICKJS_MAX_STACK_KB,
    ) {
        match value {
            Ok(value) => profile.stack_kb = value,
            Err(reason) => push_code_isolation_issue(compile_issues, node_id, "stack_kb", reason),
        }
    }

    enforce_fixed_isolation_string(
        compile_issues,
        node_id,
        &mut profile.network,
        isolation.get("network"),
        "network",
        CodeIsolationProfile::DEFAULT_NETWORK,
    );
    enforce_fixed_isolation_string(
        compile_issues,
        node_id,
        &mut profile.filesystem,
        isolation.get("filesystem"),
        "filesystem",
        CodeIsolationProfile::DEFAULT_FILESYSTEM,
    );
    enforce_fixed_isolation_string(
        compile_issues,
        node_id,
        &mut profile.env,
        isolation.get("env"),
        "env",
        CodeIsolationProfile::DEFAULT_ENV,
    );
    enforce_fixed_isolation_string(
        compile_issues,
        node_id,
        &mut profile.secrets,
        isolation.get("secrets"),
        "secrets",
        CodeIsolationProfile::DEFAULT_SECRETS,
    );

    profile
}

fn isolation_string(value: Option<&Value>) -> Option<Result<&str, &'static str>> {
    let value = value?;
    Some(
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("value must be a non-empty string"),
    )
}

fn bounded_u64(value: Option<&Value>, max: u64) -> Option<Result<u64, &'static str>> {
    let value = value?;
    match value.as_u64() {
        Some(number) if number > 0 && number <= max => Some(Ok(number)),
        Some(_) => Some(Err("value is outside the supported hard limit")),
        None => Some(Err("value must be a positive integer")),
    }
}

fn bounded_u32(value: Option<&Value>, max: u32) -> Option<Result<u32, &'static str>> {
    bounded_u64(value, u64::from(max)).map(|result| result.map(|value| value as u32))
}

fn enforce_fixed_isolation_string(
    compile_issues: &mut Vec<CompileIssue>,
    node_id: &str,
    target: &mut String,
    value: Option<&Value>,
    field: &'static str,
    allowed: &'static str,
) {
    let Some(value) = isolation_string(value) else {
        return;
    };
    match value {
        Ok(value) if value == allowed => {
            *target = value.to_string();
        }
        Ok(_) => {
            push_code_isolation_issue(
                compile_issues,
                node_id,
                field,
                "resource access must remain denied for local code execution",
            );
        }
        Err(reason) => push_code_isolation_issue(compile_issues, node_id, field, reason),
    }
}

fn push_code_isolation_issue(
    compile_issues: &mut Vec<CompileIssue>,
    node_id: &str,
    field: &str,
    reason: &str,
) {
    compile_issues.push(CompileIssue {
        field_path: None,
        node_id: node_id.to_string(),
        code: CompileIssueCode::InvalidCodeIsolationProfile,
        message: format!("code isolation profile field `{field}` is invalid: {reason}"),
    });
}

pub(super) fn trimmed_config_string<'a>(config: &'a Value, key: &str) -> Option<&'a str> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn code_import_aliases(config: &Value) -> Vec<String> {
    config
        .get("imports")
        .and_then(Value::as_array)
        .map(|imports| {
            imports
                .iter()
                .filter_map(|import| {
                    import
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn validate_code_imports(
    node_id: &str,
    config: &Value,
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) {
    let Some(imports) = config.get("imports") else {
        return;
    };
    let Some(imports) = imports.as_array() else {
        compile_issues.push(CompileIssue {
            field_path: None,
            node_id: node_id.to_string(),
            code: CompileIssueCode::InvalidJsDependencyImport,
            message: format!("node {node_id} config.imports must be an array of alias strings"),
        });
        return;
    };

    for (index, import) in imports.iter().enumerate() {
        let Some(alias) = import
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            compile_issues.push(CompileIssue {
                field_path: None,
                node_id: node_id.to_string(),
                code: CompileIssueCode::InvalidJsDependencyImport,
                message: format!(
                    "node {node_id} config.imports[{index}] must be a non-empty alias string"
                ),
            });
            continue;
        };
        let target = "backend_code";
        let key = js_dependency_lookup_key(target, alias);
        if !context.js_dependencies.contains_key(&key) {
            compile_issues.push(CompileIssue {
            field_path: None,
                node_id: node_id.to_string(),
                code: CompileIssueCode::JsDependencyImportNotEnabled,
                message: format!(
                    "node {node_id} imports alias {alias} for target {target}, but it is not enabled"
                ),
            });
        }
    }
}
