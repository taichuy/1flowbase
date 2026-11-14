use super::*;
fn context(id: &str) -> ApiOperationContext {
    let declaration = DECLARATIONS
        .iter()
        .find(|item| item.interface_id == id)
        .unwrap();
    ApiOperationContext {
        interface_id: id.into(),
        method: declaration.method.into(),
        path: declaration.path.into(),
    }
}

#[test]
fn json_archive_is_a_real_json_contract_in_both_phases() {
    let c = context("system_templates.preview");
    let input =
        TemplateInput::ResolvePreview(super::super::super::catalog::TemplateRequest::Archive(
            super::super::super::catalog::ArchiveRequest {
                archive_base64: "UEsDBAoAAAA=".into(),
            },
        ));
    assert!(TemplateInput::document_input_unavailable(&c).is_none());
    assert_eq!(
        input.project_document_input(&c).unwrap(),
        json!({"body":{"archive_base64":"UEsDBAoAAAA="}})
    );
    let c = context("system_templates.export");
    let output = TemplateOutput(
        json!({"archive_base64":"UEsDBAoAAAA=","file_name":"application-template.zip"}),
    );
    assert!(TemplateOutput::document_output_unavailable(&c).is_none());
    assert_eq!(output.project_document_output(&c).unwrap(), output.0);
}
#[test]
fn template_handoff_retains_query_fields_before_normalization() {
    let c = context("system_templates.catalog");
    let input = TemplateInput::CatalogQuery(super::super::super::catalog::CatalogQuery {
        category: None,
        cursor: Some("next".into()),
        q: Some("actual query".into()),
    });
    assert_eq!(
        input.project_document_input(&c).unwrap(),
        json!({"query":{"cursor":"next","q":"actual query"}})
    );
    let c = context("system_templates.export");
    let input = TemplateInput::ExportQuery {
        selection: PortableTemplateSelection::default(),
        format: Some("json".into()),
    };
    assert_eq!(
        input.project_document_input(&c).unwrap()["query"],
        json!({"format":"json"})
    );
}
