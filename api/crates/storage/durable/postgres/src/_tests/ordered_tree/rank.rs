use crate::ordered_tree::rank::{between, rebalance, FractionalRank};

fn rank(value: &str) -> FractionalRank {
    FractionalRank::parse(value).expect("fixture rank should be canonical")
}

// AC-007: normal allocation remains strictly between its supplied neighbors.
#[test]
fn fractional_rank_between_prepend_and_append_preserve_byte_order() {
    let left = rank("F");
    let right = rank("k");
    let middle = between(Some(&left), Some(&right)).unwrap().rank;
    let prepended = between(None, Some(&left)).unwrap().rank;
    let appended = between(Some(&right), None).unwrap().rank;

    assert!(prepended < left);
    assert!(left < middle);
    assert!(middle < right);
    assert!(right < appended);
    assert_eq!(
        middle.cmp(&right),
        middle.as_str().as_bytes().cmp(right.as_str().as_bytes())
    );
}

// AC-007: insertion order can repeatedly target any neighboring pair without renumbering.
#[test]
fn fractional_rank_random_insertion_matrix_stays_strict_and_canonical() {
    let mut seed = 0x6a09_e667_f3bc_c909_u64;
    let mut ranks = Vec::<FractionalRank>::new();
    for _ in 0..512 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let insertion = (seed as usize) % (ranks.len() + 1);
        let allocation = between(
            insertion.checked_sub(1).and_then(|index| ranks.get(index)),
            ranks.get(insertion),
        )
        .unwrap();
        assert!(!allocation.rank.as_str().ends_with('0'));
        ranks.insert(insertion, allocation.rank);
    }

    assert!(ranks.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(ranks
        .windows(2)
        .all(|pair| { pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes() }));
}

// AC-007: rebalance is deterministic and emits a compact strictly increasing replacement batch.
#[test]
fn fractional_rank_rebalance_is_deterministic_strict_and_canonical() {
    let first = rebalance(1_024).unwrap();
    let second = rebalance(1_024).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.len(), 1_024);
    assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(first.iter().all(|value| !value.as_str().ends_with('0')));
}

// The prefix allocation is persisted; keeping only the order is not enough.
#[test]
fn fractional_rank_zero_prefix_preserves_exact_bytes_and_rebalance_boundary() {
    for (prefix, zero_count) in [("", 30), ("", 31), ("", 4_096), ("A", 29), ("A", 30)] {
        let zeros = "0".repeat(zero_count);
        let left = (!prefix.is_empty()).then(|| rank(prefix));
        for (digit, before) in [('1', "0U"), ('2', "1"), ('9', "4"), ('A', "5"), ('z', "U")] {
            let right = rank(&format!("{prefix}{zeros}{digit}tail"));
            let allocation = between(left.as_ref(), Some(&right)).unwrap();
            let expected = format!("{prefix}{zeros}{before}");
            assert_eq!(allocation.rank.as_str(), expected);
            assert_eq!(allocation.rebalance_recommended, expected.len() > 32);
            assert!(left.as_ref().is_none_or(|left| left < &allocation.rank));
            assert!(allocation.rank < right);
        }
    }
}
