use super::*;
use axum::extract::{rejection::JsonRejection, FromRequest, Request};
use axum::Json;
use std::future::Future;

// This private macro expands seven concrete decode functions, never a generic decoder.
macro_rules! body_extractor {
    ($body:ident, $decode:ident) => {
        pub(crate) async fn $decode(request: Request) -> Result<$body, JsonRejection> {
            let Json(body) = <Json<$body> as FromRequest<()>>::from_request(request, &()).await?;
            Ok(body)
        }

        impl<S: Send + Sync> FromRequest<S> for $body {
            type Rejection = JsonRejection;

            fn from_request(
                request: Request,
                _state: &S,
            ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
                $decode(request)
            }
        }
    };
}

body_extractor!(CreateModelDefinitionBody, decode_create_model_definition);
body_extractor!(UpdateModelDefinitionBody, decode_update_model_definition);
body_extractor!(CreateModelFieldBody, decode_create_model_field);
body_extractor!(UpdateModelFieldBody, decode_update_model_field);
body_extractor!(CreateScopeGrantBody, decode_create_scope_grant);
body_extractor!(UpdateScopeGrantBody, decode_update_scope_grant);
body_extractor!(BatchDeleteModelDefinitionsBody, decode_batch_delete_models);
