use crate::config;

use base64::{STANDARD, STANDARD_NO_PAD, URL_SAFE_NO_PAD, decode_config};
use jwt_simple::algorithms::MACLike;
use jwt_simple::prelude::{HS256Key, NoCustomClaims, VerificationOptions};

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub(crate) struct AuthState {
    key: HS256Key,
    issuer: String,
    cookie_name: String,
}

#[derive(Debug)]
pub(crate) enum AuthError {
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
        }))
    }

    pub(crate) fn cookie_name(&self) -> &str {
        &self.cookie_name
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
