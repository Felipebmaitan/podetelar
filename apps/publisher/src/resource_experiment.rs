//! Opt-in counters; observing never retains pixels or media owners.
use eframe::egui::ColorImage;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
static STARTED: AtomicU64 = AtomicU64::new(0);
static FINISHED: AtomicU64 = AtomicU64::new(0);
static POOLS: OnceLock<Mutex<HashMap<u64, (usize, usize)>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FREEDOMSTREAM_DEEP_OWNERS").is_ok_and(|v| v == "1"))
}
pub struct VideoTask;
impl VideoTask {
    pub fn start() -> Option<Self> {
        if enabled() {
            STARTED.fetch_add(1, Ordering::Relaxed);
            Some(Self)
        } else {
            None
        }
    }
}
impl Drop for VideoTask {
    fn drop(&mut self) {
        FINISHED.fetch_add(1, Ordering::Relaxed);
    }
}
// Weak image references would disable Arc::get_mut and change the experiment.
// Observe pool accounting instead, without adding any Arc/Weak pixel owners.
pub struct Pool(Option<u64>);
impl Default for Pool {
    fn default() -> Self {
        if enabled() {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            POOLS
                .get_or_init(Default::default)
                .lock()
                .unwrap()
                .insert(id, (0, 0));
            Self(Some(id))
        } else {
            Self(None)
        }
    }
}
impl Pool {
    pub fn update(&self, images: &[Arc<ColorImage>]) {
        if let Some(id) = self.0 {
            POOLS.get().unwrap().lock().unwrap().insert(
                id,
                (
                    images.len(),
                    images.iter().map(|i| i.pixels.len() * 4).sum(),
                ),
            );
        }
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        if let Some(id) = self.0 {
            POOLS.get().unwrap().lock().unwrap().remove(&id);
        }
    }
}
pub fn snapshot() -> serde_json::Value {
    let pools = POOLS.get_or_init(Default::default).lock().unwrap();
    serde_json::json!({"enabled":enabled(),"videoTasksStarted":STARTED.load(Ordering::Relaxed),"videoTasksCompleted":FINISHED.load(Ordering::Relaxed),"rgbaPools":pools.len(),"rgbaPoolImages":pools.values().map(|v|v.0).sum::<usize>(),"rgbaPoolBytes":pools.values().map(|v|v.1).sum::<usize>(),"rgbaGl":gl_snapshot(),"defaultHeap":if enabled(){heap_snapshot()}else{serde_json::Value::Null}})
}

pub fn video_tasks_drained() -> bool {
    enabled() && STARTED.load(Ordering::Relaxed) == FINISHED.load(Ordering::Relaxed)
}

// cargo rustc enables this private observer only with a matching local Glow copy.
// Ordinary builds, including --all-features, require no patched Glow API.
#[allow(unexpected_cfgs)]
pub fn tag_rgba_texture(id: eframe::egui::TextureId) {
    #[cfg(freedomstream_deep_gl)]
    eframe::egui_glow::deep_gl_observer::tag_video(id);
    #[cfg(not(freedomstream_deep_gl))]
    let _ = id;
}
#[allow(unexpected_cfgs)]
pub fn gl_snapshot() -> serde_json::Value {
    #[cfg(freedomstream_deep_gl)]
    {
        let v = eframe::egui_glow::deep_gl_observer::snapshot();
        serde_json::json!({"calls":v[0],"payloadBytes":v[1],"partialCalls":v[2],"taggedTextures":v[3],"tagOverflow":v[4]})
    }
    #[cfg(not(freedomstream_deep_gl))]
    serde_json::Value::Null
}

#[cfg(windows)]
mod heap {
    use std::{ffi::c_void, sync::OnceLock};
    #[repr(C)]
    #[derive(Default)]
    pub struct Summary {
        cb: u32,
        pub allocated: usize,
        pub committed: usize,
        pub reserved: usize,
        pub max_reserve: usize,
    }
    type Query = unsafe extern "system" fn(*mut c_void, u32, *mut Summary) -> i32;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetModuleHandleW"]
        fn module_handle(name: *const u16) -> *mut c_void;
        #[link_name = "GetProcAddress"]
        fn proc_address(module: *mut c_void, name: *const u8) -> *const c_void;
        #[link_name = "GetProcessHeap"]
        fn process_heap() -> *mut c_void;
        #[link_name = "HeapSetInformation"]
        fn set_information(
            heap: *mut c_void,
            class: i32,
            info: *const c_void,
            length: usize,
        ) -> i32;
    }
    pub fn summary() -> Option<Summary> {
        static QUERY: OnceLock<Option<Query>> = OnceLock::new();
        let query = QUERY.get_or_init(|| unsafe {
            let module = module_handle(windows::core::w!("kernel32.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let address = proc_address(module, c"HeapSummary".as_ptr().cast());
            if address.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*const c_void, Query>(address))
            }
        });
        let query = query.as_ref()?;
        let mut value = Summary {
            cb: std::mem::size_of::<Summary>() as u32,
            ..Default::default()
        };
        // SAFETY: the process default heap remains valid for this process. The
        // SDK-matching repr(C) buffer is initialized with its size and flags=0.
        (unsafe { query(process_heap(), 0, &mut value) } != 0).then_some(value)
    }
    pub fn optimize() -> bool {
        #[repr(C)]
        struct Info {
            version: u32,
            flags: u32,
        }
        let info = Info {
            version: 1,
            flags: 0,
        };
        // SAFETY: documented HeapOptimizeResources on our serialized default
        // heap, with the required version/flags. No live blocks are freed and
        // no working-set operation, heap cap or system setting is involved.
        unsafe {
            set_information(
                process_heap(),
                3,
                (&info as *const Info).cast(),
                std::mem::size_of::<Info>(),
            ) != 0
        }
    }
}
#[cfg(windows)]
pub fn heap_snapshot() -> serde_json::Value {
    match heap::summary() {
        Some(v) => {
            serde_json::json!({"supported":true,"allocatedBytes":v.allocated,"committedBytes":v.committed,"reservedBytes":v.reserved,"maxReserveBytes":v.max_reserve})
        }
        None => serde_json::json!({"supported":false}),
    }
}
#[cfg(not(windows))]
pub fn heap_snapshot() -> serde_json::Value {
    serde_json::Value::Null
}
pub fn optimize_idle_heap() -> serde_json::Value {
    let before = heap_snapshot();
    #[cfg(windows)]
    let eligible = heap::summary()
        .is_some_and(|v| v.committed.saturating_sub(v.allocated) >= 16 * 1024 * 1024);
    #[cfg(not(windows))]
    let eligible = false;
    let start = std::time::Instant::now();
    #[cfg(windows)]
    let succeeded = eligible && heap::optimize();
    #[cfg(not(windows))]
    let succeeded = false;
    serde_json::json!({"eligible":eligible,"succeeded":succeeded,"wallMs":start.elapsed().as_secs_f64()*1000.0,"before":before,"after":heap_snapshot()})
}

#[cfg(windows)]
pub fn native_snapshot() -> serde_json::Value {
    type Getter = unsafe extern "C" fn(*mut u64, usize);
    static GETTER: OnceLock<Option<Getter>> = OnceLock::new();
    let getter = GETTER.get_or_init(|| {
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            #[link_name = "GetModuleHandleW"]
            fn module_handle(name: *const u16) -> *mut c_void;
            #[link_name = "GetProcAddress"]
            fn proc_address(module: *mut c_void, name: *const u8) -> *const c_void;
        }
        // SAFETY: lookup is confined to this executable. Only the isolated SDK
        // exports this fixed signature; ordinary pinned builds return None.
        unsafe {
            let module = module_handle(std::ptr::null());
            if module.is_null() {
                return None;
            }
            let address = proc_address(module, c"freedomstream_deep_metrics".as_ptr().cast());
            if address.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*const c_void, Getter>(address))
            }
        }
    });
    let Some(getter) = getter else {
        return serde_json::Value::Null;
    };
    let mut values = [0u64; 27];
    // SAFETY: the isolated experimental SDK writes at most this array's length.
    unsafe { getter(values.as_mut_ptr(), values.len()) };
    let names = [
        "devicesCreated",
        "devicesLive",
        "samplesLive",
        "samplesPeak",
        "deferredLive",
        "planeBytesLive",
        "stagingLive",
        "readbacks",
        "readbackLockNs",
        "deviceLockNs",
        "copySubmitNs",
        "mapWallNs",
        "conversionNs",
        "mapCalls",
        "mapBusy",
        "blockingFallbacks",
        "materializations",
        "failures",
        "arraySamples",
        "singleSamples",
        "boundedSuccess",
        "genericFallback",
        "mapErrors",
        "directRenderLogs",
        "outputCopyLogs",
        "poolFullLogs",
        "traceMessages",
    ];
    serde_json::Value::Object(
        names
            .into_iter()
            .zip(values)
            .map(|(n, v)| (n.into(), v.into()))
            .collect(),
    )
}
#[cfg(not(windows))]
pub fn native_snapshot() -> serde_json::Value {
    serde_json::Value::Null
}
