use crate::types::push::Subscription;

pub trait PushSender: Clone + Send + Sync + 'static {
    type Error: std::fmt::Display + Send + Sync + 'static;
    type Fut<'a>: Future<Output = Result<(), Self::Error>> + Send + 'a
    where
        Self: 'a;

    fn send<'a>(&'a self, subscription: &'a Subscription, message: &'a str) -> Self::Fut<'a>;
}
