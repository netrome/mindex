use std::time::Duration;

use time::OffsetDateTime;

pub trait TimeProvider: Clone + Send + Sync + 'static {
    type Sleep<'a>: Future<Output = ()> + Send + 'a
    where
        Self: 'a;

    fn now(&self) -> OffsetDateTime;
    fn sleep<'a>(&'a self, duration: Duration) -> Self::Sleep<'a>;
}
