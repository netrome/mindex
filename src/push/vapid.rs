use crate::config;
use crate::push_types::VapidConfig;

#[derive(Debug, Clone)]
pub(crate) enum VapidConfigStatus {
    Missing,
    Incomplete,
    Ready(VapidConfig),
}

pub(crate) fn load_vapid_config(config: &config::AppConfig) -> VapidConfigStatus {
    let private_key = config.vapid_private_key.as_ref();
    let public_key = config.vapid_public_key.as_ref();
    let subject = config.vapid_subject.as_ref();
    let has_any = private_key.is_some() || public_key.is_some() || subject.is_some();

    match (private_key, public_key, subject) {
        (Some(private_key), Some(public_key), Some(subject)) => {
            VapidConfigStatus::Ready(VapidConfig {
                private_key: private_key.clone(),
                public_key: public_key.clone(),
                subject: subject.clone(),
            })
        }
        _ if has_any => VapidConfigStatus::Incomplete,
        _ => VapidConfigStatus::Missing,
    }
}
