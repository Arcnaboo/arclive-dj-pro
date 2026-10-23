use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct PerfCounters {
    pub callback_ns_max: AtomicU64,
    pub underruns: AtomicU64,
    pub loads: AtomicU64,
}

impl PerfCounters {
    pub fn record_callback_ns(&self, ns: u64) {
        let mut cur = self.callback_ns_max.load(Ordering::Relaxed);
        while ns > cur {
            match self
                .callback_ns_max
                .compare_exchange_weak(cur, ns, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(v) => cur = v,
            }
        }
    }
}