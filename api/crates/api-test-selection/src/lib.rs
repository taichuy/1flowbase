//! Internal API test partition attributes. No production/runtime responsibilities.
extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

const ROOTS: [(&str, &str); 4] = [
    ("core", "api_server"),
    ("contracts", "api_private_contracts"),
    ("public_api", "api_public_api"),
    ("routes", "api_private_routes"),
];

fn selected(owners: TokenStream) -> Result<bool, String> {
    let mut labels = Vec::new();
    let mut tokens = owners.into_iter();
    loop {
        let label = match tokens.next() {
            Some(TokenTree::Ident(value)) => value.to_string(),
            _ => return Err("expected a nonempty comma-separated owner list".into()),
        };
        if !ROOTS.iter().any(|(owner, _)| *owner == label) {
            return Err(format!("unknown owner {label:?}"));
        }
        if labels.contains(&label) {
            return Err(format!("duplicate owner {label:?}"));
        }
        labels.push(label);
        match tokens.next() {
            None => break,
            Some(TokenTree::Punct(value))
                if value.as_char() == ',' && value.spacing() == Spacing::Alone => {}
            _ => return Err("owner labels must be separated by one comma".into()),
        }
    }
    let name = std::env::var("CARGO_CRATE_NAME")
        .map_err(|_| "CARGO_CRATE_NAME is required for API test selection".to_string())?;
    let owner = ROOTS
        .iter()
        .find_map(|(owner, root)| (*root == name).then_some(*owner))
        .ok_or_else(|| format!("unsupported CARGO_CRATE_NAME {name:?}"))?;
    Ok(labels.iter().any(|label| label == owner))
}

fn diagnostic(message: String) -> TokenStream {
    let mut output = TokenStream::new();
    output.extend([
        TokenTree::Ident(Ident::new("compile_error", Span::call_site())),
        TokenTree::Punct(Punct::new('!', Spacing::Alone)),
        TokenTree::Group(Group::new(
            Delimiter::Brace,
            TokenStream::from(TokenTree::Literal(Literal::string(&format!(
                "api-test-selection: {message}"
            )))),
        )),
    ]);
    output
}

/// Keep the original item only in a listed standard Cargo test root.
/// Labels are core, contracts, public_api and routes; trailing commas are rejected.
#[proc_macro_attribute]
pub fn group(owners: TokenStream, item: TokenStream) -> TokenStream {
    match selected(owners) {
        Ok(true) => item,
        Ok(false) => TokenStream::new(),
        Err(message) => diagnostic(message),
    }
}

/// Always retain the original item, suppressing only its non-owner dead-code lint.
#[proc_macro_attribute]
pub fn fixture(owners: TokenStream, item: TokenStream) -> TokenStream {
    match selected(owners) {
        Ok(true) => item,
        Ok(false) => {
            let mut output: TokenStream = "#[allow(dead_code)]"
                .parse()
                .expect("fixed dead-code attribute must parse");
            output.extend(item);
            output
        }
        Err(message) => diagnostic(message),
    }
}
