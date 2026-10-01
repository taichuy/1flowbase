use super::Accounting;
#[test]
fn nested_exclusive_cost_is_not_double_counted() {
    let mut a = Accounting::default();
    assert_eq!(a.enter(1, 0, 100), None);
    assert_eq!(a.enter(2, 1, 150), Some((0, 50)));
    assert_eq!(a.exit(2, 180), (Some((1, 30)), true));
    assert_eq!(a.exit(1, 200), (Some((0, 20)), true));
}
#[test]
fn suspended_future_charges_no_wait_or_other_thread_cpu() {
    let mut a = Accounting::default();
    a.enter(1, 0, 10);
    assert_eq!(a.exit(1, 20).0, Some((0, 10)));
    assert_eq!(a.enter(1, 0, 100000), None);
    assert_eq!(a.exit(1, 100007).0, Some((0, 7)));
    let mut other = Accounting::default();
    assert_eq!(other.enter(1, 0, 500), None);
    assert_eq!(other.exit(1, 509).0, Some((0, 9)));
}
#[test]
fn bad_exit_resets_stack_and_reports_invalid() {
    let mut a = Accounting::default();
    a.enter(1, 2, 0);
    assert_eq!(a.exit(2, 5), (Some((2, 5)), false));
    assert_eq!(a.enter(3, 4, 100), None);
}
