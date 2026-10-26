//! Opt-in Linux GNU test allocator. Requested layouts exclude allocator rounding,
//! native allocations and temporary storage inside realloc; jemalloc stats and
//! sampled native profiles expose those separately. No production hooks.
use serde_json::{json, Value};
use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::{CStr, CString},
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst},
    time::{Duration, Instant},
};

struct CountingJemalloc;
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
static ALLOCATED: AtomicU64 = AtomicU64::new(0);
static FREED: AtomicU64 = AtomicU64::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static STAGE: AtomicU64 = AtomicU64::new(0);
static STACK_ENABLED: AtomicBool = AtomicBool::new(false);
static STACK_THRESHOLD: AtomicU64 = AtomicU64::new(96 * 1024 * 1024);
static CAPTURE_BUSY: AtomicBool = AtomicBool::new(false);
static CAPTURE_LIVE: AtomicU64 = AtomicU64::new(0);
static CAPTURE_STAGE: AtomicU64 = AtomicU64::new(0);
static CAPTURE_COUNT: AtomicU64 = AtomicU64::new(0);
static CAPTURE_PCS: [AtomicU64; 128] = [const { AtomicU64::new(0) }; 128];

unsafe extern "C" {
    fn backtrace(buffer: *mut *mut std::ffi::c_void, size: std::ffi::c_int) -> std::ffi::c_int;
}

fn native_frames() -> ([*mut std::ffi::c_void; 128], usize) {
    let mut frames = [std::ptr::null_mut(); 128];
    // SAFETY: GNU backtrace writes at most the supplied number of pointer slots.
    // It is warmed before measurement; no Rust formatting/allocation is used.
    let count = unsafe { backtrace(frames.as_mut_ptr(), frames.len() as std::ffi::c_int) };
    (frames, count.max(0) as usize)
}

#[global_allocator]
static ALLOCATOR: CountingJemalloc = CountingJemalloc;

fn add(size: usize) {
    let live = LIVE.fetch_add(size as u64, SeqCst) + size as u64;
    ALLOCATED.fetch_add(size as u64, SeqCst);
    let previous = PEAK.fetch_max(live, SeqCst);
    // A sampled profile can miss a short-lived high-water allocation. Retain
    // its exact calling stack above this diagnostic threshold (not a limit).
    // Jemalloc has returned before this hook; this is never called under its
    // allocation locks and never invokes mallctl/profile dumping recursively.
    if STACK_ENABLED.load(SeqCst)
        && live > previous
        && live >= STACK_THRESHOLD.load(SeqCst)
        && CAPTURE_BUSY
            .compare_exchange(false, true, SeqCst, SeqCst)
            .is_ok()
    {
        let (frames, count) = native_frames();
        if live >= CAPTURE_LIVE.load(SeqCst) {
            for (slot, frame) in CAPTURE_PCS.iter().zip(&frames[..count]) {
                slot.store(*frame as usize as u64, SeqCst);
            }
            CAPTURE_COUNT.store(count as u64, SeqCst);
            CAPTURE_STAGE.store(STAGE.load(SeqCst), SeqCst);
            CAPTURE_LIVE.store(live, SeqCst);
        }
        CAPTURE_BUSY.store(false, SeqCst);
    }
}

fn subtract(size: usize) {
    LIVE.fetch_sub(size as u64, SeqCst);
    FREED.fetch_add(size as u64, SeqCst);
}

// SAFETY: layouts/pointers are forwarded unchanged to the production allocator.
// Bookkeeping uses only atomics and never allocates or calls profiling controls.
unsafe impl GlobalAlloc for CountingJemalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { tikv_jemallocator::Jemalloc.alloc(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { tikv_jemallocator::Jemalloc.alloc_zeroed(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { tikv_jemallocator::Jemalloc.dealloc(ptr, layout) };
        subtract(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { tikv_jemallocator::Jemalloc.realloc(ptr, layout, size) };
        if !result.is_null() {
            if size >= layout.size() {
                add(size - layout.size());
            } else {
                subtract(layout.size() - size);
            }
        }
        result
    }
}

fn read_ctl<T: Copy + Default>(name: &CStr) -> T {
    let mut value = T::default();
    let mut size = std::mem::size_of::<T>();
    // SAFETY: each caller supplies the documented mallctl result type and a
    // valid NUL-terminated static name. The output points to initialized storage.
    let code = unsafe {
        tikv_jemalloc_sys::mallctl(
            name.as_ptr(),
            (&mut value as *mut T).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    assert_eq!(code, 0, "mallctl {name:?}");
    assert_eq!(size, std::mem::size_of::<T>());
    value
}

fn refresh_epoch() {
    let mut epoch = 1_u64;
    // SAFETY: epoch is a documented read/write uint64_t control.
    let code = unsafe {
        tikv_jemalloc_sys::mallctl(
            c"epoch".as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            (&mut epoch as *mut u64).cast(),
            std::mem::size_of::<u64>(),
        )
    };
    assert_eq!(code, 0, "refresh jemalloc epoch");
}

fn dump_profile(path: &Path) {
    let path = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    let mut pointer = path.as_ptr();
    // SAFETY: prof.dump consumes the filename synchronously; both the pointer
    // slot and its NUL-terminated pointee remain alive through the call.
    let code = unsafe {
        tikv_jemalloc_sys::mallctl(
            c"prof.dump".as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            (&mut pointer as *mut *const std::ffi::c_char).cast(),
            std::mem::size_of_val(&pointer),
        )
    };
    assert_eq!(code, 0, "dump jemalloc profile");
}

fn proc_fields(path: &str, fields: &[&str]) -> Value {
    let body = std::fs::read_to_string(path).unwrap();
    let mut result = serde_json::Map::new();
    for line in body.lines() {
        if let Some((key, value)) = line.split_once(':') {
            if fields.contains(&key) {
                let kib: u64 = value.split_whitespace().next().unwrap().parse().unwrap();
                result.insert(format!("{key}_KiB"), json!(kib));
            }
        }
    }
    Value::Object(result)
}

pub(super) struct Probe {
    root: PathBuf,
    started: Instant,
    output: File,
    sampler: Option<std::thread::JoinHandle<()>>,
    native_profiles: bool,
    performance: bool,
}

impl Probe {
    pub(super) fn new(root: PathBuf) -> Self {
        std::fs::create_dir_all(&root).unwrap();
        let mode = std::env::var("HISTORY_MEMORY_MODE").unwrap_or_else(|_| "performance".into());
        assert!(matches!(
            mode.as_str(),
            "performance" | "proof" | "stack" | "canary"
        ));
        let native_profiles = matches!(mode.as_str(), "stack" | "canary");
        let stack_threshold = if native_profiles {
            std::env::var("HISTORY_MEMORY_STACK_THRESHOLD_BYTES")
                .map(|value| value.parse::<u64>().expect("diagnostic stack threshold"))
                .unwrap_or(96 * 1024 * 1024)
        } else {
            96 * 1024 * 1024
        };
        STACK_THRESHOLD.store(stack_threshold, SeqCst);
        STACK_ENABLED.store(native_profiles, SeqCst);
        if native_profiles {
            std::hint::black_box(native_frames());
        }
        assert!(read_ctl::<bool>(c"config.stats"));
        assert!(read_ctl::<bool>(c"config.prof"));
        assert_eq!(
            read_ctl::<bool>(c"opt.prof"),
            native_profiles,
            "use prof:false for performance/proof, prof:true for stack/canary"
        );
        if native_profiles {
            assert!(read_ctl::<bool>(c"prof.active"));
        }
        let version = read_ctl::<*const std::ffi::c_char>(c"version");
        // SAFETY: jemalloc returns a process-lifetime NUL-terminated version.
        let version = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let identity = json!({
            "pid": std::process::id(),
            "start_ticks": stat.rsplit_once(')').unwrap().1.split_whitespace().nth(19),
            "executable": std::env::current_exe().unwrap(),
            "allocator": "CountingJemalloc -> tikv_jemallocator::Jemalloc (prefixed/static)",
            "jemalloc_version": version,
            "mode":mode,"native_profiles":native_profiles,
            "canary_in_this_process":mode == "canary",
            "sampler_thread":native_profiles,
            "stack_threshold_bytes":stack_threshold,
            "counter_overhead":"identical process-wide atomic live/peak/allocated/freed counters in all modes and builds",
            "lg_prof_sample": read_ctl::<usize>(c"opt.lg_prof_sample"),
            "background_thread": read_ctl::<bool>(c"opt.background_thread"),
            "requested_scope": "process-wide Rust GlobalAlloc layouts; not usable size or allocator resident; realloc counts net layout change",
            "profile_scope": "jemalloc native sampled live stacks; sampled snapshots can lag exact hook peak",
            "peak_stack_scope": "stack/canary only: GNU libc backtrace, prewarmed, fixed atomic slots; explicit diagnostic threshold (default 96MiB); no allocation/content limit",
            "rss_scope": "OS status and smaps_rollup; VmHWM is lifetime only, never a phase peak",
        });
        std::fs::write(
            root.join("identity.json"),
            serde_json::to_vec_pretty(&identity).unwrap(),
        )
        .unwrap();
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(root.join("stages.jsonl"))
            .unwrap();
        let started = Instant::now();
        let sample_root = root.clone();
        STOP.store(false, SeqCst);
        let sampler = native_profiles.then(|| std::thread::spawn(move || {
            let mut output = File::create(sample_root.join("samples.jsonl")).unwrap();
            let mut stage = u64::MAX;
            let mut last_profile_live = 0;
            let mut ordinal = 0;
            while !STOP.load(SeqCst) {
                let current_stage = STAGE.load(SeqCst);
                if current_stage != stage {
                    stage = current_stage;
                    last_profile_live = LIVE.load(SeqCst);
                }
                let live = LIVE.load(SeqCst);
                let peak = PEAK.load(SeqCst);
                writeln!(output, "{}", json!({"us":started.elapsed().as_micros(),"stage":stage,"requested_live":live,"requested_peak":peak})).unwrap();
                if live > last_profile_live + 8 * 1024 * 1024 {
                    let filename = format!("sample-{ordinal:03}-stage-{stage}.heap");
                    dump_profile(&sample_root.join(&filename));
                    writeln!(output, "{}", json!({"us":started.elapsed().as_micros(),"stage":stage,"profile":filename,"requested_live_before_dump":live})).unwrap();
                    last_profile_live = live;
                    ordinal += 1;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }));
        Self {
            root,
            started,
            output,
            sampler,
            native_profiles,
            performance: mode == "performance",
        }
    }

    pub(super) fn is_performance(&self) -> bool {
        self.performance
    }

    pub(super) fn begin(&mut self, name: &str) {
        STAGE.fetch_add(1, SeqCst);
        PEAK.store(LIVE.load(SeqCst), SeqCst);
        CAPTURE_LIVE.store(0, SeqCst);
        CAPTURE_COUNT.store(0, SeqCst);
        self.point(&format!("{name}:begin"), false);
    }

    pub(super) fn point(&mut self, name: &str, profile: bool) {
        // Read hook counters before the allocations used to format this sample.
        let live = LIVE.load(SeqCst);
        let peak = PEAK.load(SeqCst);
        let allocated = ALLOCATED.load(SeqCst);
        let freed = FREED.load(SeqCst);
        let peak_stack_live = CAPTURE_LIVE.load(SeqCst);
        let peak_stack_stage = CAPTURE_STAGE.load(SeqCst);
        let peak_stack_pcs = CAPTURE_PCS[..CAPTURE_COUNT.load(SeqCst) as usize]
            .iter()
            .map(|value| value.load(SeqCst))
            .collect::<Vec<_>>();
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
        let stat = stat
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .collect::<Vec<_>>();
        refresh_epoch();
        let statistics = json!({
            "allocated": read_ctl::<usize>(c"stats.allocated"),
            "active": read_ctl::<usize>(c"stats.active"),
            "resident": read_ctl::<usize>(c"stats.resident"),
            "mapped": read_ctl::<usize>(c"stats.mapped"),
            "retained": read_ctl::<usize>(c"stats.retained"),
        });
        writeln!(self.output, "{}", json!({
            "us":self.started.elapsed().as_micros(),"stage":STAGE.load(SeqCst),"name":name,
            "rust_requested_live":live,"rust_requested_peak":peak,
            "rust_requested_allocated_total":allocated,"rust_requested_freed_total":freed,
            "peak_stack_live":peak_stack_live,"peak_stack_stage":peak_stack_stage,"peak_stack_pcs":peak_stack_pcs,
            "process_cpu_ticks":{"user":stat[11].parse::<u64>().unwrap(),"system":stat[12].parse::<u64>().unwrap()},
            "jemalloc":statistics,
            "status":proc_fields("/proc/self/status", &["VmRSS","VmHWM","RssAnon","RssFile"]),
            "smaps":proc_fields("/proc/self/smaps_rollup", &["Rss","Anonymous","Private_Dirty","Pss_Anon"]),
        })).unwrap();
        if profile && self.native_profiles {
            dump_profile(
                &self
                    .root
                    .join(format!("point-{}-{name}.heap", STAGE.load(SeqCst))),
            );
        }
    }

    pub(super) fn canary(&mut self) {
        assert_eq!(
            std::env::var("HISTORY_MEMORY_MODE").as_deref(),
            Ok("canary"),
            "calibration must run in its own canary process"
        );
        const SIZE: usize = 32 * 1024 * 1024;
        self.begin("canary");
        let before = LIVE.load(SeqCst);
        let allocated_before = ALLOCATED.load(SeqCst);
        let mut canary = vec![0_u8; SIZE];
        for page in canary.chunks_mut(4096) {
            page[0] = 0xa5;
        }
        std::hint::black_box(&mut canary);
        let held = LIVE.load(SeqCst);
        let allocated_held = ALLOCATED.load(SeqCst);
        self.point("canary-held", true);
        let freed_before_drop = FREED.load(SeqCst);
        drop(canary);
        let released = LIVE.load(SeqCst);
        let freed_after_drop = FREED.load(SeqCst);
        self.point("canary-released", true);
        let result = json!({
            "size":SIZE,"before":before,"held":held,"released":released,
            "allocated_before":allocated_before,"allocated_held":allocated_held,
            "freed_before_drop":freed_before_drop,"freed_after_drop":freed_after_drop,
        });
        std::fs::write(
            self.root.join("canary.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        // A process-wide live delta can be slightly smaller than the canary if
        // the sampling thread frees startup storage concurrently. Monotonic
        // allocation/free totals cannot hide that allocation or its release.
        assert!(
            allocated_held >= allocated_before + SIZE as u64,
            "Rust canary allocation was not observed"
        );
        assert!(
            freed_after_drop >= freed_before_drop + SIZE as u64,
            "Rust canary free was not observed"
        );
        assert!(
            released < before + 1024 * 1024,
            "Rust canary did not release"
        );
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        STOP.store(true, SeqCst);
        if let Some(sampler) = self.sampler.take() {
            sampler.join().unwrap();
        }
    }
}
