use uuid::Uuid;

use crate::frontstage::default_block_title;

#[test]
fn ac_001_default_block_title_is_a_stable_eight_character_identifier() {
    let first = Uuid::parse_str("018f4b32-78a1-7d5e-9b1c-a1b2c3d4e5f6").expect("valid UUID");
    let second = Uuid::parse_str("018f4b32-78a1-7d5e-9b1c-a1b2c3d4e5f7").expect("valid UUID");

    let title = default_block_title(first);

    assert_eq!(title.len(), 8);
    assert!(title
        .bytes()
        .all(|character| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&character)));
    assert_eq!(title, default_block_title(first));
    assert_ne!(title, default_block_title(second));
}
