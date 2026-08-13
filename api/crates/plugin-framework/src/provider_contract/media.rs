use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const MEDIA_REF_PREFIX: &str = "media_sha256_";
const MEDIA_REF_DIGEST_LEN: usize = 24;

/// Returns a deterministic opaque reference for a canonical provider media block.
/// The reference is only an index: callers must resolve it against media already
/// visible in the current invocation rather than treating it as global storage.
pub fn provider_media_content_ref(block: &Value) -> Option<String> {
    let block_type = block.get("type").and_then(Value::as_str)?;
    if !matches!(
        block_type,
        "image" | "image_url" | "input_image" | "document"
    ) {
        return None;
    }

    let canonical = canonical_json(block);
    let encoded = serde_json::to_vec(&canonical).ok()?;
    let digest = format!("{:x}", Sha256::digest(encoded));
    Some(format!(
        "{MEDIA_REF_PREFIX}{}",
        &digest[..MEDIA_REF_DIGEST_LEN]
    ))
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(key, _)| *key);
            let mut canonical = Map::new();
            for (key, value) in entries {
                canonical.insert(key.clone(), canonical_json(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn media_ref_is_stable_across_object_key_order() {
        let first = json!({
            "type": "image",
            "source": { "type": "base64", "media_type": "image/png", "data": "aW1hZ2U=" }
        });
        let second: Value = serde_json::from_str(
            r#"{"source":{"data":"aW1hZ2U=","media_type":"image/png","type":"base64"},"type":"image"}"#,
        )
        .expect("fixture should be valid JSON");

        assert_eq!(
            provider_media_content_ref(&first),
            provider_media_content_ref(&second)
        );
    }

    #[test]
    fn non_media_block_has_no_media_ref() {
        assert_eq!(
            provider_media_content_ref(&json!({ "type": "text", "text": "hello" })),
            None
        );
    }
}
