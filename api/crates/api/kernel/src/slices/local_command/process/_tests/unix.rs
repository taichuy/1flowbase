use super::*;
use std::io::{Cursor, Read};

fn errno(code: i32) -> io::Error {
    io::Error::from_raw_os_error(code)
}

#[test]
fn group_first_permission_denial_is_an_error_even_after_leader_reap() {
    for reaped in [false, true] {
        let state = GroupState::default();
        state.leader_reaped.store(reaped, Ordering::Release);
        let error = state.signal_result(Err(errno(libc::EPERM))).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
        assert!(!state.signalled.load(Ordering::Acquire));
        assert!(!state.absent.load(Ordering::Acquire));
    }
}

#[test]
fn group_signal_success_without_reap_does_not_hide_permission_denial() {
    let state = GroupState::default();
    state.signal_result(Ok(())).unwrap();
    let error = state.signal_result(Err(errno(libc::EPERM))).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::EPERM));
    assert!(!state.absent.load(Ordering::Acquire));
}

#[test]
fn group_post_reap_permission_denial_never_claims_group_is_empty() {
    let state = GroupState::default();
    state.signal_result(Ok(())).unwrap();
    state.leader_reaped.store(true, Ordering::Release);
    state.signal_result(Err(errno(libc::EPERM))).unwrap();
    assert!(!state.absent.load(Ordering::Acquire));
    assert!(!state.probe_result(Err(errno(libc::EPERM))));
    assert!(!state.probe_result(Ok(())));
    // Only the OS reporting no such group establishes cleanup_complete.
    assert!(state.probe_result(Err(errno(libc::ESRCH))));
    assert!(state.absent.load(Ordering::Acquire));
}

#[test]
fn group_unrelated_post_reap_errors_still_propagate() {
    let state = GroupState::default();
    state.signal_result(Ok(())).unwrap();
    state.leader_reaped.store(true, Ordering::Release);
    let error = state.signal_result(Err(errno(libc::EINVAL))).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::EINVAL));
    assert!(!state.absent.load(Ordering::Acquire));
}

#[test]
fn collector_preserves_reaped_status_and_output_with_uncertain_group_cleanup() {
    let state = GroupState::default();
    // Controlled syscall sequence: initial SIGKILL succeeds, wait reaps the shell,
    // repeated SIGKILL is denied, then kill(0) cannot establish group absence.
    state.signal_result(Ok(())).unwrap();
    let result = collect(
        Cursor::new(b"stdout before cancellation"),
        Cursor::new(b"stderr before cancellation"),
        Spool::new().unwrap(),
        Spool::new().unwrap(),
        &|| state.signal_result(Err(errno(libc::EPERM))),
        || {
            state.leader_reaped.store(true, Ordering::Release);
            Ok((None, Some(libc::SIGKILL)))
        },
    )
    .unwrap();
    assert_eq!(result.exit_code, None);
    assert_eq!(result.signal, Some(libc::SIGKILL));
    assert!(!state.probe_result(Err(errno(libc::EPERM))));
    assert!(!result.cleanup_complete);
    for (spool, expected) in [
        (result.stdout, "stdout before cancellation"),
        (result.stderr, "stderr before cancellation"),
    ] {
        let mut text = String::new();
        spool
            .into_file()
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, expected);
    }
}

#[test]
fn collector_does_not_hide_first_cleanup_permission_error() {
    let state = GroupState::default();
    let result = collect(
        Cursor::new(b"stdout"),
        Cursor::new(b"stderr"),
        Spool::new().unwrap(),
        Spool::new().unwrap(),
        &|| state.signal_result(Err(errno(libc::EPERM))),
        || {
            state.leader_reaped.store(true, Ordering::Release);
            Ok((Some(0), None))
        },
    );
    assert_eq!(result.err().unwrap().raw_os_error(), Some(libc::EPERM));
    assert!(!state.absent.load(Ordering::Acquire));
}
