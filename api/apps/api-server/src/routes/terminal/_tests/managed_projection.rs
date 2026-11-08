use super::*;
use crate::extension_bus::FrozenManagedProjection;
use extension_contracts::ManagedProjectionContract;
use interface_runtime::{
    CompiledInterfaceRegistry, ManagedInterfaceFreezeFuture, ManagedInterfaceFreezeRequest,
    ManagedInterfaceInvocationFactory, ManagedInterfaceProjection, RegistryCompilationError,
    RegistryCompiler,
};

const SECRET: &str = "terminal-private-canary";

fn verify<T: InterfaceContract>(value: &T, expected: Value) {
    let projection = ManagedInterfaceProjection::from_contract(value)
        .expect("every terminal variant has an explicit safe projection");
    let contract = ManagedProjectionContract {
        contract_id: projection.contract().contract_id().into(),
        contract_version: projection.contract().version().into(),
        schema: projection.schema().clone(),
    };
    let frozen = FrozenManagedProjection::compile(contract).expect("canonical schema registration");
    let reference = frozen
        .validate_projection(&projection)
        .expect("typed projection matches schema");
    assert_eq!(reference.value, expected);
    let wire = serde_json::to_string(&reference.value).unwrap();
    assert!(
        !wire.contains(SECRET),
        "paths, command text, data and record fields remain private"
    );
    assert!(
        wire.len() < 1024,
        "projection size does not scale with content"
    );

    let mut leaked = reference.clone();
    leaked.value["path"] = json!(SECRET);
    assert!(
        frozen.validate_reference(&leaked).is_err(),
        "closed object rejects content injection"
    );
    let tag = if reference.value.get("action").is_some() {
        "action"
    } else {
        "kind"
    };
    let mut wrong_kind = reference.clone();
    wrong_kind.value[tag] = json!(SECRET);
    assert!(
        frozen.validate_reference(&wrong_kind).is_err(),
        "finite discriminator rejects arbitrary text"
    );
    let mut missing = reference.clone();
    missing.value.as_object_mut().unwrap().remove(tag);
    assert!(
        frozen.validate_reference(&missing).is_err(),
        "discriminator is required"
    );
    for (name, value) in reference.value.as_object().unwrap() {
        let mut omitted = reference.clone();
        omitted.value.as_object_mut().unwrap().remove(name);
        assert!(
            frozen.validate_reference(&omitted).is_err(),
            "{name} is required"
        );
        if name == "state" {
            let mut changed = reference.clone();
            changed.value[name] = json!(SECRET);
            assert!(
                frozen.validate_reference(&changed).is_err(),
                "job state is a finite trusted enum"
            );
        }
        if value.is_number() {
            for bad in [json!(-1), json!(SECRET)] {
                let mut changed = reference.clone();
                changed.value[name] = bad;
                assert!(
                    frozen.validate_reference(&changed).is_err(),
                    "{name} must be a nonnegative count"
                );
            }
        } else if value.is_boolean() {
            let mut changed = reference.clone();
            changed.value[name] = json!(1);
            assert!(
                frozen.validate_reference(&changed).is_err(),
                "{name} remains boolean"
            );
        }
    }
}

#[test]
fn terminal_managed_input_projection_validates_all_actions_without_sensitive_arguments() {
    macro_rules! input {
        ($variant:ident, $value:expr) => {
            TerminalInput::$variant(serde_json::from_value($value).unwrap())
        };
    }
    let id = Uuid::nil();
    let cases = vec![
        (input!(Info, json!({})), json!({"action":"info"})),
        (
            input!(
                List,
                json!({"path":SECRET,"timeout_ms":20,"cursor":1,"limit":3})
            ),
            json!({"action":"files.list","timeout_ms":20,"limit":3}),
        ),
        (
            input!(Stat, json!({"path":SECRET,"timeout_ms":20})),
            json!({"action":"files.stat","timeout_ms":20}),
        ),
        (
            input!(
                Text,
                json!({"path":SECRET,"timeout_ms":20,"offset":1,"max_bytes":30,"expected_version":SECRET})
            ),
            json!({"action":"files.read_text","timeout_ms":20,"offset":1,"max_bytes":30}),
        ),
        (
            input!(
                Range,
                json!({"path":SECRET,"timeout_ms":20,"offset":1,"length":30,"expected_version":SECRET})
            ),
            json!({"action":"files.read_range","timeout_ms":20,"offset":1,"length":30}),
        ),
        (
            input!(
                Search,
                json!({"path":SECRET,"timeout_ms":20,"pattern":SECRET,"max_scan_bytes":100,"max_matches":3,"max_depth":2,"max_line_bytes":30})
            ),
            json!({"action":"files.search","timeout_ms":20,"pattern_bytes":SECRET.len(),"max_scan_bytes":100,"max_matches":3,"max_depth":2,"max_line_bytes":30}),
        ),
        (
            input!(
                Image,
                json!({"path":SECRET,"timeout_ms":20,"max_file_bytes":100})
            ),
            json!({"action":"files.read_image","timeout_ms":20,"max_file_bytes":100}),
        ),
        (
            input!(
                Audio,
                json!({"path":SECRET,"timeout_ms":20,"max_file_bytes":100})
            ),
            json!({"action":"files.read_audio","timeout_ms":20,"max_file_bytes":100}),
        ),
        (
            input!(
                Resource,
                json!({"path":SECRET,"timeout_ms":20,"max_file_bytes":100,"mime_type":SECRET,"link_only":true})
            ),
            json!({"action":"files.read_resource","timeout_ms":20,"max_file_bytes":100,"link_only":true}),
        ),
        (
            input!(Import, json!({"resource_uri":SECRET,"file_table_id":id})),
            json!({"action":"files.import_resource"}),
        ),
        (
            input!(
                Download,
                json!({"file_table_id":id,"record_id":id,"target_path":SECRET,"expected_sha256":SECRET,"timeout_ms":20,"overwrite":true})
            ),
            json!({"action":"files.download_file","timeout_ms":20,"overwrite":true}),
        ),
        (
            input!(
                Start,
                json!({"request_id":id,"command":SECRET.repeat(100_000),"timeout_ms":20,"retention_ms":30,"max_output_bytes":100})
            ),
            json!({"action":"jobs.start","command_bytes":SECRET.len()*100_000,"timeout_ms":20,"retention_ms":30,"max_output_bytes":100}),
        ),
        (
            input!(Get, json!({"execution_id":id})),
            json!({"action":"jobs.get"}),
        ),
        (
            input!(
                Read,
                json!({"execution_id":id,"stream":"stderr","offset":1,"max_bytes":30})
            ),
            json!({"action":"jobs.read","offset":1,"max_bytes":30}),
        ),
        (
            input!(Cancel, json!({"execution_id":id})),
            json!({"action":"jobs.cancel"}),
        ),
    ];
    assert_eq!(cases.len(), INTERFACE_IDS.len());
    for (value, expected) in cases {
        verify(&value, expected);
    }
}

fn metadata() -> FileMetadata {
    FileMetadata {
        name: SECRET.into(),
        path: SECRET.into(),
        kind: SECRET.into(),
        size: 123,
        modified_at: Some(10),
        permissions: Some(0o600),
        version: SECRET.into(),
    }
}

#[test]
fn terminal_managed_output_projection_validates_all_variants_without_sensitive_results() {
    let secret_data = || SECRET.repeat(100_000);
    let cases = vec![
        (
            TerminalOutput::Info(InfoOutput {
                os: SECRET.into(),
                shell: SECRET.into(),
                execution_location: SECRET.into(),
                capabilities: vec![SECRET.into()],
                pty: false,
            }),
            json!({"kind":"info","capability_count":1,"pty":false}),
        ),
        (
            TerminalOutput::List(ListOutput {
                path: SECRET.into(),
                entries: vec![metadata()],
                next_cursor: 1,
                eof: true,
            }),
            json!({"kind":"list","entry_count":1,"eof":true}),
        ),
        (
            TerminalOutput::Stat(metadata()),
            json!({"kind":"stat","file_bytes":123}),
        ),
        (
            TerminalOutput::Text(TextOutput {
                path: SECRET.into(),
                offset: 0,
                text: secret_data(),
                next_offset: 20,
                eof: false,
                version: SECRET.into(),
            }),
            json!({"kind":"text","text_bytes":SECRET.len()*100_000,"eof":false}),
        ),
        (
            TerminalOutput::Range(RangeOutput {
                data: secret_data(),
                mime_type: SECRET.into(),
                metadata: RangeMetadata {
                    path: SECRET.into(),
                    offset: 0,
                    byte_count: 12,
                    next_offset: 12,
                    eof: true,
                    version: SECRET.into(),
                },
            }),
            json!({"kind":"range","byte_count":12,"eof":true}),
        ),
        (
            TerminalOutput::Search(SearchOutput {
                matches: vec![SearchMatch {
                    path: SECRET.into(),
                    line_number: 1,
                    byte_offset: 0,
                    text: secret_data(),
                }],
                scanned_bytes: 123,
                truncated: true,
                skipped_long_lines: 2,
                skipped_binary_files: 3,
                skipped_special_files: 4,
                depth_limited: true,
            }),
            json!({"kind":"search","match_count":1,"scanned_bytes":123,"truncated":true,"skipped_long_lines":2,"skipped_binary_files":3,"skipped_special_files":4,"depth_limited":true}),
        ),
        (
            TerminalOutput::Media(MediaOutput {
                data: secret_data(),
                mime_type: SECRET.into(),
                metadata: metadata(),
            }),
            json!({"kind":"media","encoded_bytes":SECRET.len()*100_000,"file_bytes":123}),
        ),
        (
            TerminalOutput::Resource(ResourceOutput {
                uri: SECRET.into(),
                name: SECRET.into(),
                data: Some(secret_data()),
                mime_type: SECRET.into(),
                metadata: metadata(),
            }),
            json!({"kind":"resource","encoded_bytes":SECRET.len()*100_000,"file_bytes":123,"link_only":false}),
        ),
        (
            TerminalOutput::Resource(ResourceOutput {
                uri: SECRET.into(),
                name: SECRET.into(),
                data: None,
                mime_type: SECRET.into(),
                metadata: metadata(),
            }),
            json!({"kind":"resource","encoded_bytes":0,"file_bytes":123,"link_only":true}),
        ),
        (
            TerminalOutput::Import(ImportOutput {
                file_table_id: SECRET.into(),
                storage_id: SECRET.into(),
                record: json!({SECRET:secret_data(),"nested":{"private":SECRET}}),
            }),
            json!({"kind":"import","record_field_count":2}),
        ),
        (
            TerminalOutput::Download(DownloadOutput {
                file_table_id: Uuid::nil(),
                record_id: Uuid::nil(),
                target_path: SECRET.into(),
                byte_count: 123,
                sha256: SECRET.into(),
            }),
            json!({"kind":"download","byte_count":123}),
        ),
        (
            TerminalOutput::Read(ReadOutput {
                text: secret_data(),
                next_offset: 123,
                eof: true,
                encoding_lossy: false,
            }),
            json!({"kind":"read","text_bytes":SECRET.len()*100_000,"eof":true,"encoding_lossy":false}),
        ),
    ];
    for (value, expected) in cases {
        verify(&value, expected);
    }
    for state in ["running", "completed", "failed", "cancelled", "timed_out"] {
        let job: JobSnapshot = serde_json::from_value(json!({"execution_id":Uuid::nil(),"request_id":Uuid::nil(),"state":state,"cancel_requested":true,"exit_code":null,"signal":null,"duration_ms":12,"output_complete":true,"cleanup_complete":true,"truncated":false,"discarded_bytes":1,"stdout_bytes":2,"stderr_bytes":3,"created_at_unix_ms":10,"expires_at_unix_ms":null,"retention_ms":20,"timeout_ms":30,"max_output_bytes":40})).unwrap();
        verify(
            &TerminalOutput::Job(job),
            json!({"kind":"job","state":state,"cancel_requested":true,"duration_ms":12,"output_complete":true,"cleanup_complete":true,"truncated":false,"discarded_bytes":1,"stdout_bytes":2,"stderr_bytes":3}),
        );
    }
}

struct NeverInvokedFactory;
impl ManagedInterfaceInvocationFactory for NeverInvokedFactory {
    fn freeze(&self, _: ManagedInterfaceFreezeRequest) -> ManagedInterfaceFreezeFuture<'_> {
        panic!("registry compilation must not invoke handlers");
    }
}
fn with_managed(
    snapshot: &CompiledInterfaceRegistry,
) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
    let operations = snapshot
        .definitions()
        .map(|d| d.authorization_operation().clone())
        .collect::<std::collections::BTreeSet<_>>();
    let owners = snapshot
        .definitions()
        .map(|d| d.owner().clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut compiler =
        RegistryCompiler::new(snapshot.graph_fingerprint().clone(), operations, owners)
            .with_managed_invocations(Arc::new(NeverInvokedFactory));
    compiler.absorb_snapshot(snapshot)?;
    compiler.compile()
}
struct MissingProjection;
impl InterfaceContract for MissingProjection {
    const CONTRACT_ID: &'static str = "terminal-test-missing-projection";
    const CONTRACT_VERSION: &'static str = "1";
}
// Only the managed projection is intentionally absent in this negative fixture.
impl crate::openapi_interface::contract::ApiInput for MissingProjection {
    fn document_input_schema(
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<Value> {
        Some(serde_json::json!({"type":"object","properties":{},"additionalProperties":false}))
    }
    fn project_document_input(
        &self,
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<Value, Arc<str>> {
        Ok(serde_json::json!({}))
    }
}
struct MissingProjectionAdapter;
impl ConsoleInterfacePort<MissingProjection, TerminalOutput> for MissingProjectionAdapter {
    fn execute<'a>(
        &'a self,
        _: &'a interface_runtime::UserPrincipal,
        _: MissingProjection,
    ) -> ConsoleInterfaceFuture<'a, TerminalOutput> {
        panic!("missing projection must be rejected before execution");
    }
}
#[test]
fn terminal_managed_registry_accepts_complete_contracts_and_rejects_missing_projection() {
    let snapshot = console_interface::compile_registry(
        "api-server.console-terminal",
        "graph:terminal-projection-test",
        DECLARATIONS,
        Arc::new(TerminalAdapter { state: Weak::new() }),
    )
    .unwrap();
    let registry = with_managed(&snapshot)
        .expect("all terminal interfaces publish with managed hooks enabled");
    let actual = registry
        .definitions()
        .map(|d| d.interface_id().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = INTERFACE_IDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);
    for descriptor in registry.managed_contracts() {
        FrozenManagedProjection::compile(ManagedProjectionContract {
            contract_id: descriptor.contract.contract_id().into(),
            contract_version: descriptor.contract.version().into(),
            schema: descriptor
                .schema
                .as_deref()
                .expect("complete schema")
                .clone(),
        })
        .unwrap();
    }
    // Reproduce the original missing-codec condition against the actual managed registry compiler.
    let missing = console_interface::compile_registry(
        "api-server.console-terminal",
        "graph:terminal-projection-test",
        &DECLARATIONS[..1],
        Arc::new(MissingProjectionAdapter),
    )
    .unwrap();
    assert!(matches!(
        with_managed(&missing),
        Err(RegistryCompilationError::ManagedProjectionMismatch(_))
    ));
}
