// Isolated diagnostic worktree only. No credentials or payloads are recorded.
mod accounting;
use accounting::Accounting;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use tracing::{
    Subscriber,
    span::{Attributes, Id},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};
const N: usize = 8;
const NAMES: [&str; N] = [
    "context",
    "protocol",
    "ipc",
    "stream",
    "reliable",
    "logs",
    "stream.snapshot_clone",
    "stream.interface_clone",
];
static CPU: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static ENTERS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static LIFETIME: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static CLOSED: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static INVALID: AtomicU64 = AtomicU64::new(0);
thread_local! { static LOCAL: RefCell<Accounting> = RefCell::new(Accounting::default()); }
#[repr(C)]
struct Timespec {
    seconds: i64,
    nanos: i64,
}
unsafe extern "C" {
    fn clock_gettime(clock: i32, result: *mut Timespec) -> i32;
    fn _rjem_mallctl(
        name: *const std::ffi::c_char,
        old: *mut std::ffi::c_void,
        oldlen: *mut usize,
        new: *mut std::ffi::c_void,
        newlen: usize,
    ) -> i32;
}
fn clock(clock: i32) -> u64 {
    let mut t = Timespec {
        seconds: 0,
        nanos: 0,
    };
    if unsafe { clock_gettime(clock, &mut t) } != 0 {
        INVALID.fetch_add(1, Ordering::Relaxed);
        return 0;
    }
    (t.seconds as u64) * 1_000_000_000 + t.nanos as u64
}
fn stage(name: &str) -> Option<usize> {
    let name = name.strip_prefix("gateway_cost.")?;
    NAMES.iter().position(|n| *n == name).or_else(|| {
        NAMES
            .iter()
            .position(|n| *n == name.split('.').next().unwrap_or(name))
    })
}
fn charge(cost: Option<(usize, u64)>) {
    if let Some((s, n)) = cost {
        CPU[s].fetch_add(n, Ordering::Relaxed);
    }
}
struct Lifetime {
    start: Instant,
    stage: usize,
}
pub(super) struct CostLayer;
impl<S> Layer<S> for CostLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if let Some(s) = stage(attrs.metadata().name()) {
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(Lifetime {
                    start: Instant::now(),
                    stage: s,
                });
            }
        }
    }
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(s) = ctx.span(id).and_then(|v| stage(v.metadata().name())) else {
            return;
        };
        // Trajectory and business state share a persistence primitive. Keep
        // nested trajectory write CPU charged to its log owner.
        let s = if s == 4 && LOCAL.with(|v| v.borrow().current_stage()) == Some(5) {
            5
        } else {
            s
        };
        let now = clock(3);
        charge(LOCAL.with(|v| v.borrow_mut().enter(id.into_u64(), s, now)));
        ENTERS[s].fetch_add(1, Ordering::Relaxed);
    }
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if ctx
            .span(id)
            .and_then(|v| stage(v.metadata().name()))
            .is_none()
        {
            return;
        }
        let now = clock(3);
        let (cost, valid) = LOCAL.with(|v| v.borrow_mut().exit(id.into_u64(), now));
        charge(cost);
        if !valid {
            INVALID.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            if let Some(v) = span.extensions().get::<Lifetime>() {
                LIFETIME[v.stage].fetch_add(v.start.elapsed().as_nanos() as u64, Ordering::Relaxed);
                CLOSED[v.stage].fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
pub(super) fn enabled() -> bool {
    std::env::var("GATEWAY_COST_TRACE").is_ok_and(|v| v == "1")
}
fn malloc_stats() -> serde_json::Value {
    let mut epoch = 1u64;
    let rc = unsafe {
        _rjem_mallctl(
            c"epoch".as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            (&mut epoch as *mut u64).cast(),
            8,
        )
    };
    let mut values = serde_json::Map::new();
    values.insert("epoch_return_code".into(), rc.into());
    for name in [
        "stats.allocated",
        "stats.active",
        "stats.resident",
        "stats.retained",
    ] {
        let mut n = 0usize;
        let mut len = std::mem::size_of::<usize>();
        let c = std::ffi::CString::new(name).unwrap();
        let rc = unsafe {
            _rjem_mallctl(
                c.as_ptr(),
                (&mut n as *mut usize).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        values.insert(
            name.into(),
            serde_json::json!({"bytes":if rc==0 {Some(n)}else{None},"return_code":rc}),
        );
    }
    values.into()
}
pub(super) async fn snapshot() -> axum::Json<serde_json::Value> {
    let stages:Vec<_>=NAMES.iter().enumerate().map(|(s,n)|serde_json::json!({"stage":n,"exclusive_thread_cpu_ns":CPU[s].load(Ordering::Relaxed),"poll_or_sync_enters":ENTERS[s].load(Ordering::Relaxed),"closed_spans":CLOSED[s].load(Ordering::Relaxed),"overlapping_span_lifetime_wall_ns":LIFETIME[s].load(Ordering::Relaxed)})).collect();
    axum::Json(
        serde_json::json!({"diagnostic_only":true,"pid":std::process::id(),"process_cpu_ns":clock(2),"invalid_accounting":INVALID.load(Ordering::Relaxed),"stages":stages,"jemalloc":malloc_stats(),"wall_lifetimes_are_not_cpu_or_additive":true}),
    )
}
