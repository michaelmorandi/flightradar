use std::sync::Arc;

use time::OffsetDateTime;

/// Time source. Inject a `SystemClock` in production; in tests a `FixedClock`
/// makes time deterministic.
pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> OffsetDateTime;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

pub type SharedClock = Arc<dyn Clock>;
