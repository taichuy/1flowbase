use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiSuccess<T> {
    pub data: T,
    pub meta: Option<serde_json::Value>,
}

impl<T> ApiSuccess<T> {
    pub fn new(data: T) -> Self {
        Self { data, meta: None }
    }
}
