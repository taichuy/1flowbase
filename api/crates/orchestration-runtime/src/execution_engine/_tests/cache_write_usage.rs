use super::*;
use std::collections::BTreeMap;

fn writes(total: u64, buckets: &[(&str, u64)]) -> ProviderUsage {
    ProviderUsage {
        cache_write_tokens: Some(total),
        cache_write_by_ttl_seconds: Some(
            buckets
                .iter()
                .map(|(ttl, count)| (ttl.to_string(), *count))
                .collect::<BTreeMap<_, _>>(),
        ),
        ..Default::default()
    }
}

#[test]
fn ac3_final_snapshot_does_not_repeat_streamed_cache_writes() {
    let events = vec![
        ProviderStreamEvent::UsageDelta {
            usage: writes(3000, &[("300", 3000)]),
        },
        ProviderStreamEvent::UsageDelta {
            usage: writes(2000, &[("3600", 2000)]),
        },
    ];
    let final_usage = writes(5000, &[("300", 3000), ("3600", 2000)]);
    assert_eq!(collect_usage(&events, &final_usage), final_usage);
    assert_eq!(
        collect_usage(&events, &ProviderUsage::default()),
        final_usage
    );
}

#[test]
fn ac3_partial_snapshots_preserve_unreported_ttl_buckets() {
    let events = vec![
        ProviderStreamEvent::UsageSnapshot {
            usage: writes(3000, &[("300", 3000)]),
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: writes(5000, &[("3600", 2000)]),
        },
    ];
    let final_usage = ProviderUsage {
        output_tokens: Some(100),
        ..Default::default()
    };
    let usage = collect_usage(&events, &final_usage);
    assert_eq!(usage.cache_write_tokens, Some(5000));
    assert_eq!(usage.output_tokens, Some(100));
    assert_eq!(
        usage.cache_write_by_ttl_seconds,
        writes(5000, &[("300", 3000), ("3600", 2000)]).cache_write_by_ttl_seconds
    );
}
