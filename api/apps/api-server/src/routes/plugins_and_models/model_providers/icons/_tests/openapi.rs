use super::*;
use serde_json::Value;

#[test]
fn provider_icon_openapi_covers_actual_image_media_and_binary_fallback_without_json() {
    let operation =
        serde_json::to_value(<__path_read_provider_icon as utoipa::Path>::operation()).unwrap();
    let content = operation["responses"]["200"]["content"]
        .as_object()
        .unwrap();
    let media = [
        "fixture.svg",
        "fixture.png",
        "fixture.jpg",
        "fixture.jpeg",
        "fixture.webp",
        "fixture.gif",
        "fixture.unknown",
    ]
    .into_iter()
    .map(|path| provider_icon_content_type(FsPath::new(path)))
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        content
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        media
    );
    assert!(!content.contains_key("application/json"));
    let schema = crate::event_center::api_projection::schema::<crate::openapi::OpenApiBinaryBody>();
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["format"], "binary");
    for media in content.values() {
        assert!(media.get("schema").is_some_and(Value::is_object));
    }
}
