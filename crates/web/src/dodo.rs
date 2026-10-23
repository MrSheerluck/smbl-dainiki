use std::{
    env,
    io::{Error as IoError, ErrorKind},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;
use topcoat::Result;

const TEST_API_BASE: &str = "https://test.dodopayments.com";
const LIVE_API_BASE: &str = "https://live.dodopayments.com";
const WEBHOOK_TOLERANCE_SECS: i64 = 300;

#[derive(Clone, Debug)]
pub struct Config {
    pub api_base: String,
    pub api_key: String,
    pub monthly_product_id: String,
    pub yearly_product_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BillingMode {
    Dodo,
    Mock,
    Disabled,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutSession {
    pub session_id: String,
    pub checkout_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PortalSession {
    pub link: String,
}

pub fn mode() -> BillingMode {
    if env::var("BILLING_MODE").as_deref() == Ok("mock") {
        let base = app_base_url();
        if !(base.starts_with("http://127.0.0.1") || base.starts_with("http://localhost")) {
            eprintln!("[billing] refusing mock billing outside loopback HTTP");
            return BillingMode::Disabled;
        }

        return BillingMode::Mock;
    }

    if config().is_some() {
        BillingMode::Dodo
    } else {
        BillingMode::Disabled
    }
}

pub fn config() -> Option<Config> {
    let api_key = env::var("DODO_PAYMENTS_API_KEY").ok()?;
    let monthly_product_id = env::var("DODO_MONTHLY_PRODUCT_ID").ok()?;
    let yearly_product_id = env::var("DODO_YEARLY_PRODUCT_ID").ok()?;

    let api_base = env::var("DODO_PAYMENTS_API_BASE_URL").unwrap_or_else(|_| {
        if env::var("DODO_PAYMENTS_ENVIRONMENT").as_deref() == Ok("live_mode") {
            LIVE_API_BASE.to_string()
        } else {
            TEST_API_BASE.to_string()
        }
    });

    Some(Config {
        api_base: api_base.trim_end_matches('/').to_string(),
        api_key,
        monthly_product_id,
        yearly_product_id,
    })
}

pub fn product_id<'a>(config: &'a Config, plan: &str) -> &'a str {
    match plan {
        "yearly" => &config.yearly_product_id,
        _ => &config.monthly_product_id,
    }
}

pub async fn create_checkout(
    config: &Config,
    user_id: i64,
    email: &str,
    plan: &str,
    return_url: &str,
    cancel_url: &str,
) -> Result<CheckoutSession> {
    let body = serde_json::json!({
        "product_cart": [{
            "product_id": product_id(config, plan),
            "quantity": 1
        }],
        "customer": {
            "email": email
        },
        "metadata": {
            "user_id": user_id.to_string(),
            "plan": plan
        },
        "return_url": return_url,
        "cancel_url": cancel_url,
        "customization": {
            "theme": "system"
        },
        "feature_flags": {
            "redirect_immediately": true
        }
    });

    let response = reqwest::Client::new()
        .post(format!("{}/checkouts", config.api_base))
        .bearer_auth(&config.api_key)
        .json(&body)
        .send()
        .await?;

    parse_response(response, "checkout session").await
}

pub async fn create_portal(
    config: &Config,
    customer_id: &str,
    return_url: &str,
) -> Result<PortalSession> {
    let response = reqwest::Client::new()
        .post(format!(
            "{}/customers/{customer_id}/customer-portal/session",
            config.api_base
        ))
        .bearer_auth(&config.api_key)
        .query(&[("return_url", return_url)])
        .send()
        .await?;

    parse_response(response, "customer portal session").await
}

pub async fn cancel_subscription(config: &Config, subscription_id: &str) -> Result<()> {
    let response = reqwest::Client::new()
        .patch(format!(
            "{}/subscriptions/{subscription_id}",
            config.api_base
        ))
        .bearer_auth(&config.api_key)
        .json(&serde_json::json!({
            "status": "cancelled",
            "cancel_reason": "cancelled_by_customer"
        }))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(IoError::new(
            ErrorKind::Other,
            format!("Dodo subscription cancellation failed ({status}): {detail}"),
        )
        .into());
    }

    Ok(())
}

pub async fn get_subscription(
    config: &Config,
    subscription_id: &str,
) -> Result<serde_json::Value> {
    let response = reqwest::Client::new()
        .get(format!(
            "{}/subscriptions/{subscription_id}",
            config.api_base
        ))
        .bearer_auth(&config.api_key)
        .send()
        .await?;

    parse_response(response, "subscription").await
}

async fn parse_response<T>(response: reqwest::Response, label: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(IoError::new(
            ErrorKind::Other,
            format!("Dodo {label} request failed ({status}): {body}"),
        )
        .into());
    }

    Ok(serde_json::from_str(&body)?)
}

pub fn verify_webhook(
    webhook_id: &str,
    webhook_timestamp: &str,
    webhook_signature: &str,
    body: &[u8],
) -> Result<bool> {
    let secret = env::var("DODO_PAYMENTS_WEBHOOK_KEY").map_err(|_| {
        IoError::new(
            ErrorKind::NotFound,
            "DODO_PAYMENTS_WEBHOOK_KEY is not configured",
        )
    })?;

    verify_webhook_with_secret(
        &secret,
        webhook_id,
        webhook_timestamp,
        webhook_signature,
        body,
    )
}

fn verify_webhook_with_secret(
    secret: &str,
    webhook_id: &str,
    webhook_timestamp: &str,
    webhook_signature: &str,
    body: &[u8],
) -> Result<bool> {
    let timestamp: i64 = webhook_timestamp.parse().map_err(|_| {
        IoError::new(ErrorKind::InvalidData, "webhook timestamp is not a number")
    })?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();

    if (now - timestamp).abs() > WEBHOOK_TOLERANCE_SECS {
        return Ok(false);
    }

    let secret = secret.strip_prefix("whsec_").unwrap_or(&secret);
    let key = STANDARD
        .decode(secret)
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "webhook secret is not base64"))?;

    let mut mac = Hmac::<Sha256>::new_from_slice(&key)
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "invalid webhook secret"))?;
    mac.update(webhook_id.as_bytes());
    mac.update(b".");
    mac.update(webhook_timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    let expected = STANDARD.encode(mac.finalize().into_bytes());

    for candidate in webhook_signature.split_whitespace() {
        let Some(candidate) = candidate.strip_prefix("v1,") else {
            continue;
        };

        if constant_time_eq(candidate.as_bytes(), expected.as_bytes()) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn app_base_url() -> String {
    env::var("APP_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &str, id: &str, timestamp: &str, body: &[u8]) -> String {
        let key = STANDARD.decode(secret).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(&key).unwrap();
        mac.update(id.as_bytes());
        mac.update(b".");
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(body);
        format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn accepts_a_valid_standard_webhooks_signature() {
        let secret = "d2hvb2stc2VjcmV0";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let body = br#"{"type":"subscription.active"}"#;
        let signature = sign(secret, "evt_1", &timestamp, body);

        assert!(
            verify_webhook_with_secret(
                &format!("whsec_{secret}"),
                "evt_1",
                &timestamp,
                &signature,
                body
            )
            .unwrap()
        );
    }

    #[test]
    fn rejects_tampered_payloads_and_old_timestamps() {
        let secret = "d2hvb2stc2VjcmV0";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let body = br#"{"type":"subscription.active"}"#;
        let signature = sign(secret, "evt_1", &timestamp, body);
        let secret = format!("whsec_{secret}");

        assert!(
            !verify_webhook_with_secret(&secret, "evt_1", &timestamp, &signature, b"tampered")
                .unwrap()
        );

        let old = (timestamp.parse::<i64>().unwrap() - 600).to_string();
        let old_signature = sign(secret.trim_start_matches("whsec_"), "evt_1", &old, body);
        assert!(!verify_webhook_with_secret(&secret, "evt_1", &old, &old_signature, body).unwrap());
    }
}
