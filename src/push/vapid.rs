use base64::{URL_SAFE_NO_PAD, encode_config};
use jwt_simple::prelude::ES256KeyPair;
use rand::rngs::OsRng;
use rand::{CryptoRng, RngCore};

use crate::config;
use crate::types::push::VapidConfig;

#[derive(Debug, Clone)]
pub(crate) struct VapidCredentials {
    pub(crate) private_key: String,
    pub(crate) public_key: String,
}

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

pub(crate) fn generate_vapid_credentials() -> Result<VapidCredentials, web_push::WebPushError> {
    let mut rng = OsRng;
    generate_vapid_credentials_with_rng(&mut rng)
}

pub(crate) fn generate_vapid_credentials_with_rng<R: RngCore + CryptoRng>(
    rng: &mut R,
) -> Result<VapidCredentials, web_push::WebPushError> {
    let key_pair = generate_es256_keypair_with_rng(rng);
    let private_key = encode_config(key_pair.to_bytes(), URL_SAFE_NO_PAD);
    let public_key =
        web_push::VapidSignatureBuilder::from_base64_no_sub(&private_key, URL_SAFE_NO_PAD)?
            .get_public_key();
    let public_key = encode_config(public_key, URL_SAFE_NO_PAD);

    Ok(VapidCredentials {
        private_key,
        public_key,
    })
}

fn generate_es256_keypair_with_rng<R: RngCore + CryptoRng>(rng: &mut R) -> ES256KeyPair {
    let mut key_bytes = [0u8; 32];
    loop {
        rng.fill_bytes(&mut key_bytes);
        if let Ok(key_pair) = ES256KeyPair::from_bytes(&key_bytes) {
            return key_pair;
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn generate_vapid_credentials_with_rng__should_return_expected_fixture() {
        // Given
        let seed = [7u8; 32];
        let mut rng = StdRng::from_seed(seed);

        // When
        let credentials =
            generate_vapid_credentials_with_rng(&mut rng).expect("credentials should generate");

        // Then
        assert_eq!(
            credentials.private_key,
            "9pKJeIXAyyCj5M0QagsVvDYHlPF-cymJCbB5iHPsdEE"
        );
        assert_eq!(
            credentials.public_key,
            "BCRweRf_U5iQM4pKNucGRzM6OuLp8Hisa8yX0N2ePIf1oxKitvFT6qvuGgYoTxlMatMDaytXbZR3rVClc2w_p6U"
        );
    }
}
