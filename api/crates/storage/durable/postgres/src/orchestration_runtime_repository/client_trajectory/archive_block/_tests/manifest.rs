use super::*;

#[test]
fn manifest_preserves_receipt_identities_and_rejects_bad_coverage() {
    let entry = |first, last| Entry {
        id: Uuid::now_v7(),
        first,
        last,
        offset: 0,
        length: 0,
        checksum: vec![1; 32],
        locator: b"CAL1\0\x01".to_vec(),
    };
    let entries = vec![entry(1, 2), entry(3, 4)];
    let encoded = encode(&entries).unwrap();
    assert_eq!(decode(&encoded).unwrap(), entries);
    for bad in [
        encoded[..encoded.len() - 1].to_vec(),
        [encoded.clone(), vec![0]].concat(),
        b"CAS1\xff\xff\xff\xff".to_vec(),
    ] {
        assert!(decode(&bad).is_err());
    }
    assert!(encode(&[entry(1, 2), entry(4, 5)]).is_err());
    let id = Uuid::now_v7();
    let mut one = entry(1, 2);
    one.id = id;
    let mut two = entry(3, 4);
    two.id = id;
    assert!(encode(&[one, two]).is_err());
}
