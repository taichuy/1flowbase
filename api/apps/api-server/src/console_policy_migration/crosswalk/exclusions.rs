use super::{LegacyNoProjectionSpec, no_projection};

pub(super) const LEGACY_NO_PROJECTIONS: &[LegacyNoProjectionSpec] = &[
    no_projection(
        "application.manage.own",
        "No baseline console route or service gate consumed application.manage.own.",
    ),
    no_projection(
        "application.manage.all",
        "No baseline console route or service gate consumed application.manage.all.",
    ),
    no_projection(
        "external_data_source.configure.own",
        "The Core has no owner-scoped data-source simple-operation contract; do not widen it to workspace scope.",
    ),
    no_projection(
        "external_data_source.delete.own",
        "The baseline delete permission has no registered live console operation.",
    ),
    no_projection(
        "external_data_source.delete.all",
        "The baseline delete permission has no registered live console operation.",
    ),
    no_projection(
        "external_data_source.use.own",
        "External data-source use is runtime capability authorization, not a console policy operation.",
    ),
    no_projection(
        "external_data_source.use.all",
        "External data-source use is runtime capability authorization, not a console policy operation.",
    ),
    no_projection(
        "file_table.view.own",
        "File-table own scope remains the runtime/file ACL contract and has no console row policy.",
    ),
    no_projection(
        "file_table.delete.own",
        "File-table own scope remains the runtime/file ACL contract and has no console row policy.",
    ),
    no_projection(
        "state_model.view.own",
        "Model-definition own scope has no registered console resource contract; projection would silently broaden it.",
    ),
    no_projection(
        "state_model.edit.own",
        "Model-definition own scope has no registered console resource contract; projection would silently broaden it.",
    ),
    no_projection(
        "state_model.delete.own",
        "Model-definition own scope has no registered console resource contract; projection would silently broaden it.",
    ),
    no_projection(
        "state_model.manage.own",
        "State-model manage is a runtime/metadata contract without a live console operation.",
    ),
    no_projection(
        "state_model.manage.all",
        "State-model manage is a runtime/metadata contract without a live console operation.",
    ),
    no_projection(
        "workspace.view.all",
        "Workspace detail and workspace list are Authenticated console views, not role-configurable operations.",
    ),
    no_projection(
        "frontstage.page.design",
        "Frontstage page design is a frontstage/runtime authorization contract, excluded from console policy.",
    ),
    no_projection(
        "ui_block.javascript.native",
        "Native UI-block execution is runtime capability authorization, excluded from console policy.",
    ),
    no_projection(
        "flow.view.own",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.view.all",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.create.all",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.edit.own",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.edit.all",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.delete.own",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.delete.all",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.manage.own",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.manage.all",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.use.own",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "flow.use.all",
        "Flow permissions remain runtime/application authorization and are outside this console migration.",
    ),
    no_projection(
        "publish_endpoint.view.own",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.view.all",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.create.all",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.edit.own",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.edit.all",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.delete.own",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.delete.all",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.publish.own",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.publish.all",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.use.own",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "publish_endpoint.use.all",
        "Published endpoint permissions are public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "state_data.view.own",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.view.all",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.create.all",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.edit.own",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.edit.all",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.delete.own",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.delete.all",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.manage.own",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.manage.all",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.use.own",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "state_data.use.all",
        "State-data permissions are runtime data ACL, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.view.own",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.view.all",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.create.all",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.edit.own",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.edit.all",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.delete.own",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
    no_projection(
        "embedded_app.delete.all",
        "Embedded-app access is public/runtime authorization, excluded from console policy.",
    ),
];

pub(super) const LEGACY_SOURCE_RESOURCES: &[&str] = &[
    "api_reference",
    "application",
    "embedded_app",
    "external_data_source",
    "file_storage",
    "file_table",
    "flow",
    "frontstage",
    "mcp_management",
    "plugin_config",
    "publish_endpoint",
    "settings_feature",
    "state_data",
    "state_model",
    "system_runtime",
    "ui_block",
    "workspace",
];
