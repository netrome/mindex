use std::time::Duration;

use time::OffsetDateTime;

use crate::push_types::Subscription;

pub trait TimeProvider: Clone + Send + Sync + 'static {
    type Sleep<'a>: Future<Output = ()> + Send + 'a
    where
        Self: 'a;

    fn now(&self) -> OffsetDateTime;
    fn sleep<'a>(&'a self, duration: Duration) -> Self::Sleep<'a>;
}

pub trait PushSender: Clone + Send + Sync + 'static {
    type Error: std::fmt::Display + Send + Sync + 'static;
    type Fut<'a>: Future<Output = Result<(), Self::Error>> + Send + 'a
    where
        Self: 'a;

    fn send<'a>(&'a self, subscription: &'a Subscription, message: &'a str) -> Self::Fut<'a>;
}
