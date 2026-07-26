use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendBlockPermissions {
    pub network: String,
    pub storage: String,
    pub secrets: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontendBlockContextContract {
    pub primitives: Vec<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendBlockCodeModule {
    pub source: String,
    pub type_declarations: String,
    #[serde(default)]
    pub components: Vec<FrontendComponentContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontendComponentImplementationKind {
    AntdFacade,
    Custom,
}

impl FrontendComponentImplementationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AntdFacade => "antd_facade",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendComponentUpstream {
    pub package: String,
    pub component: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendComponentImplementation {
    pub kind: FrontendComponentImplementationKind,
    pub upstream: Option<FrontendComponentUpstream>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendComponentProp {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendComponentExample {
    pub title: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontendComponentContract {
    pub component_code: String,
    pub export_name: String,
    pub implementation: FrontendComponentImplementation,
    pub description: String,
    pub props: Vec<FrontendComponentProp>,
    pub limitations: Vec<String>,
    pub examples: Vec<FrontendComponentExample>,
    pub insert_snippet: String,
}

impl FrontendComponentContract {
    pub fn typescript_declaration(&self, module_source: &str) -> String {
        let mut declaration = String::new();
        declaration.push_str("declare module '");
        declaration.push_str(module_source);
        declaration.push_str("' {\n");
        declaration.push_str("  export interface ");
        declaration.push_str(&self.export_name);
        declaration.push_str("Props extends FacadeCommonProps {\n");
        for prop in &self.props {
            declaration.push_str("    /** ");
            declaration.push_str(&jsdoc_text(&prop.description));
            declaration.push_str(" */\n    readonly ");
            declaration.push_str(&prop.name);
            if !prop.required {
                declaration.push('?');
            }
            declaration.push_str(": ");
            declaration.push_str(&prop.type_name);
            declaration.push_str(";\n");
        }
        declaration.push_str("  }\n\n  /**\n   * ");
        declaration.push_str(&jsdoc_text(&self.description));
        declaration.push_str("\n   *\n   * @remarks\n");
        for limitation in &self.limitations {
            declaration.push_str("   * - ");
            declaration.push_str(&jsdoc_text(limitation));
            declaration.push('\n');
        }
        for example in &self.examples {
            declaration.push_str("   *\n   * @example ");
            declaration.push_str(&jsdoc_text(&example.title));
            declaration.push_str("\n   * ```tsx\n");
            for line in example.code.lines() {
                declaration.push_str("   * ");
                declaration.push_str(&jsdoc_text(line));
                declaration.push('\n');
            }
            declaration.push_str("   * ```\n");
        }
        if let Some(upstream) = self.implementation.upstream.as_ref() {
            declaration.push_str("   *\n   * @see ");
            declaration.push_str(&jsdoc_text(&upstream.package));
            declaration.push('@');
            declaration.push_str(&jsdoc_text(&upstream.version));
            declaration.push(' ');
            declaration.push_str(&jsdoc_text(&upstream.component));
            declaration.push('\n');
        }
        declaration.push_str("   */\n  export const ");
        declaration.push_str(&self.export_name);
        declaration.push_str(": FacadeComponent<");
        declaration.push_str(&self.export_name);
        declaration.push_str("Props>;\n}\n");
        declaration
    }
}

impl FrontendBlockCodeModule {
    pub fn resolved_type_declarations(&self) -> String {
        let mut declarations = self.type_declarations.trim_end().to_string();
        for component in &self.components {
            declarations.push_str("\n\n");
            declarations.push_str(&component.typescript_declaration(&self.source));
        }
        declarations
    }
}

fn jsdoc_text(value: &str) -> String {
    value.replace("*/", "* /").replace(['\r', '\n'], " ")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontendBlockCatalogEntry {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub title: String,
    pub runtime: String,
    pub entry: String,
    pub code_template: Option<String>,
    pub code_template_version: Option<String>,
    pub code_template_language: Option<String>,
    pub code_modules: Vec<FrontendBlockCodeModule>,
    pub context_contract: FrontendBlockContextContract,
    pub permissions: FrontendBlockPermissions,
    pub ui_capabilities: Vec<String>,
}
