use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;

use crate::ports;
use crate::push;

#[derive(Debug, Clone, Copy, Default)]
pub struct TokioTimeProvider;

impl ports::TimeProvider for TokioTimeProvider {
    type Sleep<'a>
        = tokio::time::Sleep
    where
        Self: 'a;

    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn sleep<'a>(&'a self, duration: Duration) -> Self::Sleep<'a> {
        tokio::time::sleep(duration)
    }
}

#[derive(Clone)]
pub struct WebPushSender {
    vapid: push::VapidConfig,
    client: Arc<web_push::WebPushClient>,
}

impl WebPushSender {
    pub fn new(vapid: push::VapidConfig) -> Result<Self, web_push::WebPushError> {
        let client = web_push::WebPushClient::new()?;
        Ok(Self {
            vapid,
            client: Arc::new(client),
        })
    }
}

impl ports::PushSender for WebPushSender {
    type Error = web_push::WebPushError;
    type Fut<'a>
        = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
    where
        Self: 'a;

    fn send<'a>(&'a self, subscription: &'a push::Subscription, message: &'a str) -> Self::Fut<'a> {
        Box::pin(async move {
            let subscription_info = web_push::SubscriptionInfo::new(
                subscription.endpoint.clone(),
                subscription.p256dh.clone(),
                subscription.auth.clone(),
            );
            let mut builder = web_push::WebPushMessageBuilder::new(&subscription_info)?;
            builder.set_payload(web_push::ContentEncoding::Aes128Gcm, message.as_bytes());
            let mut signature_builder = web_push::VapidSignatureBuilder::from_base64(
                &self.vapid.private_key,
                web_push::URL_SAFE_NO_PAD,
                &subscription_info,
            )?;
            signature_builder.add_claim("sub", self.vapid.subject.as_str());
            builder.set_vapid_signature(signature_builder.build()?);
            self.client.send(builder.build()?).await?;
            Ok(())
        })
    }
}
