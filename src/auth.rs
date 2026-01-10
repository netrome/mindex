use crate::config;

use base64::{STANDARD, STANDARD_NO_PAD, URL_SAFE_NO_PAD, decode_config, encode_config};
use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{
    Claims, Duration as JwtDuration, HS256Key, NoCustomClaims, VerificationOptions,
};
use rand::rngs::OsRng;
use rand::{CryptoRng, RngCore};

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub(crate) struct AuthState {
    key: HS256Key,
    issuer: String,
    cookie_name: String,
    token_ttl: time::Duration,
    cookie_secure: bool,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidKey,
    InvalidToken,
    MissingExpiry,
    MissingSubject,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidKey => f.write_str("invalid auth key"),
            AuthError::InvalidToken => f.write_str("invalid auth token"),
            AuthError::MissingExpiry => f.write_str("auth token missing expiry"),
            AuthError::MissingSubject => f.write_str("auth token missing subject"),
        }
    }
}

impl AuthState {
    pub(crate) fn from_config(config: &config::AppConfig) -> Result<Option<Self>, AuthError> {
        let Some(auth) = config.auth.as_ref() else {
            return Ok(None);
        };

        let key_bytes = decode_key(&auth.key)?;
        let key = HS256Key::from_bytes(&key_bytes);

        Ok(Some(Self {
            key,
            issuer: config.app_name.clone(),
            cookie_name: auth.cookie_name.clone(),
            token_ttl: auth.token_ttl,
            cookie_secure: auth.cookie_secure,
        }))
    }

    pub(crate) fn cookie_name(&self) -> &str {
        &self.cookie_name
    }

    pub(crate) fn issue_token(&self, subject: &str) -> Result<String, AuthError> {
        let ttl_seconds = self.token_ttl.whole_seconds();
        if ttl_seconds <= 0 {
            return Err(AuthError::InvalidToken);
        }
        let claims = Claims::create(JwtDuration::from_secs(ttl_seconds as u64))
            .with_subject(subject)
            .with_issuer(&self.issuer);
        self.key
            .authenticate(claims)
            .map_err(|_| AuthError::InvalidToken)
    }

    pub(crate) fn auth_cookie(&self, token: &str) -> String {
        let max_age = self.token_ttl.whole_seconds().max(0);
        let mut cookie = format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}",
            self.cookie_name
        );
        if self.cookie_secure {
            cookie.push_str("; Secure");
        }
        cookie
    }

    pub(crate) fn clear_cookie(&self) -> String {
        let mut cookie = format!(
            "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
            self.cookie_name
        );
        if self.cookie_secure {
            cookie.push_str("; Secure");
        }
        cookie
    }

    pub(crate) fn verify_token(&self, token: &str) -> Result<(), AuthError> {
        let mut options = VerificationOptions::default();
        let mut issuers = HashSet::new();
        issuers.insert(self.issuer.clone());
        options.allowed_issuers = Some(issuers);

        let claims = self
            .key
            .verify_token::<NoCustomClaims>(token, Some(options))
            .map_err(|_| AuthError::InvalidToken)?;

        if claims.expires_at.is_none() {
            return Err(AuthError::MissingExpiry);
        }

        let subject = claims.subject.ok_or(AuthError::MissingSubject)?;
        if subject.trim().is_empty() {
            return Err(AuthError::MissingSubject);
        }

        Ok(())
    }
}

fn decode_key(raw: &str) -> Result<Vec<u8>, AuthError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AuthError::InvalidKey);
    }

    let decoded = decode_config(trimmed, URL_SAFE_NO_PAD)
        .or_else(|_| decode_config(trimmed, STANDARD))
        .or_else(|_| decode_config(trimmed, STANDARD_NO_PAD))
        .map_err(|_| AuthError::InvalidKey)?;

    if decoded.is_empty() {
        return Err(AuthError::InvalidKey);
    }

    Ok(decoded)
}

pub fn generate_auth_key() -> Result<String, AuthError> {
    let mut rng = OsRng;
    generate_auth_key_with_rng(&mut rng)
}

pub(crate) fn generate_auth_key_with_rng<R: RngCore + CryptoRng>(
    rng: &mut R,
) -> Result<String, AuthError> {
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    let encoded = encode_config(bytes, URL_SAFE_NO_PAD);
    if encoded.is_empty() {
        return Err(AuthError::InvalidKey);
    }
    Ok(encoded)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    struct ZeroRng;

    impl RngCore for ZeroRng {
        fn next_u32(&mut self) -> u32 {
            0
        }

        fn next_u64(&mut self) -> u64 {
            0
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for value in dest.iter_mut() {
                *value = 0;
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    impl CryptoRng for ZeroRng {}

    #[test]
    fn generate_auth_key_with_rng__should_match_fixture() {
        // Given
        let mut rng = ZeroRng;

        // When
        let key = generate_auth_key_with_rng(&mut rng).expect("auth key");

        // Then
        assert_eq!(key, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }
}
