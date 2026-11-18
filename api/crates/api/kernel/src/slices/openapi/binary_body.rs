#[derive(utoipa::ToSchema)]
#[schema(value_type = String, format = Binary)]
#[allow(dead_code)]
pub struct OpenApiBinaryBody(pub Vec<u8>);
