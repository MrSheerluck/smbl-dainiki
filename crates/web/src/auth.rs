use std::{
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use topcoat::{
    Result,
    context::{Cx, app_context},
    cookie::{Cookie, Cookies, SameSite, cookies, time::Duration as CookieDuration},
    router::{
        content::Form,
        error::{SeeOther, forbidden, see_other},
        page,
        request::{client_ip, headers},
        route,
    },
    session::{self, SessionConfig, Token, TokenStore, TokenStoreFuture},
    view::{View, view},
};

use crate::{
    billing::billing_section,
    dodo::{self, BillingMode},
    mail,
};

const SESSION_LIFETIME: Duration = Duration::from_secs(60 * 60 * 24 * 30);
const SESSION_REFRESH_WINDOW_SECS: i64 = 60 * 60 * 24 * 7;
const VERIFICATION_LIFETIME_SECS: i64 = 60 * 60 * 24;
const RESET_LIFETIME_SECS: i64 = 60 * 30;
const HASH_MEMORY_KIB: u32 = 64 * 1024;
const HASH_ITERATIONS: u32 = 3;
const HASH_PARALLELISM: u32 = 1;
const MIN_PASSWORD_LENGTH: usize = 12;
const MAX_PASSWORD_LENGTH: usize = 128;

const COMMON_PASSWORDS: &[&str] = &[
    "123456789012",
    "passwordpassword",
    "correcthorsebatterystaple",
    "letmeinletmein",
    "qwertyuiop123",
    "iloveyouiloveyou",
    "dragonpassword",
    "adminadminadmin",
];

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

pub fn session_config() -> SessionConfig {
    SessionConfig::builder()
        .token_store(SessionTokenStore::new())
        .lifetime(SESSION_LIFETIME)
        .build()
}

struct SessionTokenStore {
    secure: bool,
}

impl SessionTokenStore {
    fn new() -> Self {
        Self {
            secure: secure_cookies(),
        }
    }

    fn name(&self) -> String {
        if self.secure {
            "__Host-dainiki_session".to_string()
        } else {
            "dainiki_session".to_string()
        }
    }
}

impl TokenStore for SessionTokenStore {
    fn read<'a>(&'a self, cx: &'a Cx) -> TokenStoreFuture<'a, Option<Token>> {
        Box::pin(async move {
            let Some(cookie) = cookies(cx).get(&self.name()) else {
                return Ok(None);
            };

            Ok(Token::decode(cookie.value()).ok())
        })
    }

    fn write<'a>(
        &'a self,
        cx: &'a Cx,
        token: Token,
        max_age: Duration,
    ) -> TokenStoreFuture<'a, ()> {
        Box::pin(async move {
            let mut cookie = Cookie::new(self.name(), token.encode());
            cookie.set_path("/");
            cookie.set_http_only(true);
            cookie.set_same_site(SameSite::Lax);
            cookie.set_secure(self.secure);
            cookie.set_max_age(CookieDuration::seconds(max_age.as_secs() as i64));
            cookies(cx).add(cookie);
            Ok(())
        })
    }

    fn delete<'a>(&'a self, cx: &'a Cx) -> TokenStoreFuture<'a, ()> {
        Box::pin(async move {
            cookies(cx).remove(Cookie::build((self.name(), "")).path("/").build());
            Ok(())
        })
    }
}

fn secure_cookies() -> bool {
    if let Ok(value) = std::env::var("SESSION_COOKIE_SECURE") {
        return matches!(value.as_str(), "1" | "true" | "TRUE" | "yes");
    }

    std::env::var("APP_BASE_URL")
        .map(|base| base.starts_with("https://"))
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: i64,
    pub email: String,
    pub plan: Option<String>,
    pub verified: bool,
    pub csrf_token: String,
}

#[derive(Debug, Deserialize)]
pub struct SignupForm {
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    pub plan: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SigninForm {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SignupQuery {
    plan: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SigninQuery {
    notice: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenQuery {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AccountQuery {
    notice: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ForgotForm {
    email: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResetForm {
    token: String,
    password: String,
    confirm_password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CsrfForm {
    csrf_token: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangePasswordForm {
    csrf_token: String,
    current_password: String,
    new_password: String,
    confirm_password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangeEmailForm {
    csrf_token: String,
    new_email: String,
    current_password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteAccountForm {
    csrf_token: String,
    password: String,
    confirmation: String,
}

pub async fn current_user(cx: &Cx) -> Result<Option<CurrentUser>> {
    let Some(token_hash) = session::token_hash(cx).await? else {
        return Ok(None);
    };

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let row: Option<(i64, String, Option<String>, Option<i64>, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT u.id, u.email, u.plan, u.email_verified_at, s.csrf_token, s.expires_at
             FROM sessions s
             JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = $1 AND s.expires_at > $2",
        )
        .bind(token_hash.to_vec())
        .bind(now)
        .fetch_optional(pool)
        .await?;

    let Some((id, email, plan, verified_at, csrf_token, expires_at)) = row else {
        return Ok(None);
    };

    if expires_at - now <= SESSION_REFRESH_WINDOW_SECS {
        if let Some(refreshed) = session::refresh(cx).await? {
            let new_expiry = refreshed
                .expires_at
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or(expires_at);

            sqlx::query("UPDATE sessions SET expires_at = $2 WHERE token_hash = $1")
                .bind(token_hash.to_vec())
                .bind(new_expiry)
                .execute(pool)
                .await?;
        }
    }

    let csrf_token = match csrf_token {
        Some(token) if !token.is_empty() => token,
        _ => {
            let token = random_token();
            sqlx::query("UPDATE sessions SET csrf_token = $2 WHERE token_hash = $1")
                .bind(token_hash.to_vec())
                .bind(&token)
                .execute(pool)
                .await?;
            token
        }
    };

    Ok(Some(CurrentUser {
        id,
        email,
        plan,
        verified: verified_at.is_some(),
        csrf_token,
    }))
}

pub(crate) async fn require_user(cx: &Cx) -> Result<CurrentUser> {
    match current_user(cx).await? {
        Some(user) => Ok(user),
        None => Err(see_other("/signin").into()),
    }
}

#[page("/signup")]
pub async fn signup_page(cx: &Cx, query: Option<Form<SignupQuery>>) -> Result<impl View> {
    if current_user(cx).await?.is_some() {
        return Err(see_other("/account").into());
    }

    let plan = query
        .and_then(|Form(query)| query.plan)
        .filter(|plan| matches!(plan.as_str(), "monthly" | "yearly"))
        .unwrap_or_else(|| "monthly".to_string());

    auth_signup(cx, None, String::new(), plan)
}

#[page(POST "/signup")]
pub async fn signup(cx: &Cx, Form(input): Form<SignupForm>) -> Result<impl View> {
    let plan = match normalize_plan(input.plan.as_deref()) {
        Ok(plan) => plan,
        Err(message) => {
            return auth_signup(
                cx,
                Some(message.to_string()),
                input.email,
                input.plan.unwrap_or_else(|| "monthly".to_string()),
            );
        }
    };

    let email = match normalize_email(&input.email) {
        Ok(email) => email,
        Err(message) => {
            return auth_signup(cx, Some(message.to_string()), input.email, plan.to_string());
        }
    };

    if input.password != input.confirm_password {
        return auth_signup(
            cx,
            Some("Those passwords do not match.".to_string()),
            input.email,
            plan.to_string(),
        );
    }

    if let Err(message) = validate_password(&email, &input.password) {
        return auth_signup(cx, Some(message.to_string()), input.email, plan.to_string());
    }

    let pool = app_context::<PgPool>(cx);
    let ip = client_ip_key(cx);

    if !rate_limit_allows(pool, &format!("signup:ip:{ip}"), 50, 3600).await? {
        return auth_signup(
            cx,
            Some("Too many sign-up attempts. Try again later.".to_string()),
            input.email,
            plan.to_string(),
        );
    }

    if !rate_limit_allows(
        pool,
        &format!("signup:email:{}", input.email.trim().to_lowercase()),
        5,
        3600,
    )
    .await?
    {
        return auth_signup(
            cx,
            Some("Too many sign-up attempts. Try again later.".to_string()),
            input.email,
            plan.to_string(),
        );
    }

    let password_hash = hash_password_async(input.password).await?;
    let verified_at = mail::emails_disabled().then(now_seconds);
    let inserted = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users
             (email, email_normalized, password_hash, plan, email_verified_at)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(input.email.trim())
    .bind(&email)
    .bind(password_hash)
    .bind(plan)
    .bind(verified_at)
    .fetch_one(pool)
    .await;

    match inserted {
        Ok(user_id) => {
            start_session(cx, user_id).await?;

            if !mail::emails_disabled() {
                if let Err(error) =
                    send_verification_email(cx, user_id, input.email.trim()).await
                {
                    eprintln!("[auth] could not send verification email: {error}");
                }
            }

            audit(cx, pool, Some(user_id), "signup").await;

            let notice = if mail::emails_disabled() {
                "/account?notice=email-verified"
            } else {
                "/account?notice=verification-sent"
            };

            Err(see_other(notice).into())
        }
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => auth_signup(
            cx,
            Some("An account with that email already exists.".to_string()),
            input.email,
            plan.to_string(),
        ),
        Err(error) => Err(error.into()),
    }
}

#[page("/signin")]
pub async fn signin_page(cx: &Cx, query: Option<Form<SigninQuery>>) -> Result<impl View> {
    if current_user(cx).await?.is_some() {
        return Err(see_other("/account").into());
    }

    let notice = signin_notice(query.and_then(|Form(query)| query.notice).as_deref());
    auth_signin(cx, None, notice, String::new())
}

#[page(POST "/signin")]
pub async fn signin(cx: &Cx, Form(input): Form<SigninForm>) -> Result<impl View> {
    let message = "Email or password is incorrect.";
    let email = normalize_email(&input.email);
    let pool = app_context::<PgPool>(cx);
    let ip = client_ip_key(cx);

    if !rate_limit_allows(pool, &format!("signin:ip:{ip}"), 40, 900).await? {
        return auth_signin(
            cx,
            Some("Too many attempts. Try again in a few minutes.".to_string()),
            None,
            input.email,
        );
    }

    let email_key = email
        .as_ref()
        .map(|email| email.to_string())
        .unwrap_or_else(|_| input.email.trim().to_lowercase());

    if !rate_limit_allows(pool, &format!("signin:email:{email_key}"), 10, 900).await? {
        return auth_signin(
            cx,
            Some("Too many attempts. Try again in a few minutes.".to_string()),
            None,
            input.email,
        );
    }

    let Some(email) = email.ok() else {
        verify_dummy_password(input.password).await;
        return auth_signin(cx, Some(message.to_string()), None, input.email);
    };

    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE email_normalized = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await?;

    let Some((user_id, password_hash)) = row else {
        verify_dummy_password(input.password).await;
        audit(cx, pool, None, "signin_failed").await;
        return auth_signin(cx, Some(message.to_string()), None, input.email);
    };

    if !verify_password_async(input.password, password_hash).await {
        audit(cx, pool, Some(user_id), "signin_failed").await;
        return auth_signin(cx, Some(message.to_string()), None, input.email);
    }

    sqlx::query("DELETE FROM rate_limits WHERE key = $1")
        .bind(format!("signin:email:{email}"))
        .execute(pool)
        .await?;

    start_session(cx, user_id).await?;
    audit(cx, pool, Some(user_id), "signin").await;
    Err(see_other("/account").into())
}

#[page("/verify-email")]
pub async fn verify_email(cx: &Cx, query: Option<Form<TokenQuery>>) -> Result<impl View> {
    let token = query
        .and_then(|Form(query)| query.token)
        .unwrap_or_default();

    if let Some(user_id) = consume_verification_token(cx, &token).await? {
        let pool = app_context::<PgPool>(cx);
        audit(cx, pool, Some(user_id), "email_verified").await;
        return auth_notice(
            cx,
            "Email verified",
            "Your email address is verified. You can now continue to your account.",
            "/account",
            "Go to account",
        );
    }

    auth_notice(
        cx,
        "This link has expired",
        "That verification link is invalid or has already been used. Sign in and send a new one from your account page.",
        "/signin",
        "Go to sign in",
    )
}

#[page(POST "/verify-email/resend")]
pub async fn resend_verification(
    cx: &Cx,
    Form(input): Form<CsrfForm>,
) -> Result<impl View> {
    let user = require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    if user.verified {
        return Err(see_other("/account?notice=email-verified").into());
    }

    let pool = app_context::<PgPool>(cx);
    if !rate_limit_allows(
        pool,
        &format!("verification-resend:user:{}", user.id),
        3,
        3600,
    )
    .await?
    {
        return account_view(
            cx,
            user,
            None,
            Some("Too many verification emails. Try again later.".to_string()),
        );
    }

    if let Err(error) = send_verification_email(cx, user.id, &user.email).await {
        eprintln!("[auth] could not resend verification email: {error}");
        return account_view(
            cx,
            user,
            None,
            Some("Could not send the verification email. Try again shortly.".to_string()),
        );
    }

    audit(cx, pool, Some(user.id), "verification_resent").await;
    Err(see_other("/account?notice=verification-sent").into())
}

#[page("/forgot-password")]
pub async fn forgot_password_page(cx: &Cx) -> Result<impl View> {
    auth_forgot(cx, false, String::new())
}

#[page(POST "/forgot-password")]
pub async fn forgot_password(cx: &Cx, Form(input): Form<ForgotForm>) -> Result<impl View> {
    let pool = app_context::<PgPool>(cx);
    let ip = client_ip_key(cx);
    let email = normalize_email(&input.email).ok();

    let ip_allowed = rate_limit_allows(pool, &format!("forgot:ip:{ip}"), 10, 3600).await?;
    let email_allowed = match email.as_deref() {
        Some(email) => {
            rate_limit_allows(pool, &format!("forgot:email:{email}"), 3, 3600).await?
        }
        None => true,
    };

    if ip_allowed && email_allowed {
        if let Some(email) = email.as_deref() {
            if let Some((user_id,)) =
                sqlx::query_as::<_, (i64,)>("SELECT id FROM users WHERE email_normalized = $1")
                    .bind(email)
                    .fetch_optional(pool)
                    .await?
            {
                match send_reset_email(cx, user_id, email).await {
                    Ok(()) => audit(cx, pool, Some(user_id), "password_reset_requested").await,
                    Err(error) => eprintln!("[auth] could not send reset email: {error}"),
                }
            }
        }
    }

    auth_forgot(cx, true, input.email)
}

#[page("/reset-password")]
pub async fn reset_password_page(cx: &Cx, query: Option<Form<TokenQuery>>) -> Result<impl View> {
    let token = query
        .and_then(|Form(query)| query.token)
        .unwrap_or_default();

    if !auth_token_is_valid(cx, &token, "reset_password").await? {
        return auth_reset_or_notice(cx, None, None);
    }

    auth_reset_or_notice(cx, Some(token), None)
}

#[page(POST "/reset-password")]
pub async fn reset_password(cx: &Cx, Form(input): Form<ResetForm>) -> Result<impl View> {
    if input.password != input.confirm_password {
        return auth_reset_or_notice(
            cx,
            Some(input.token),
            Some("Those passwords do not match.".to_string()),
        );
    }

    if let Err(message) = validate_password("", &input.password) {
        return auth_reset_or_notice(cx, Some(input.token), Some(message.to_string()));
    }

    let password_hash = hash_password_async(input.password).await?;
    let pool = app_context::<PgPool>(cx);
    let Some(user_id) = replace_password_with_token(cx, &input.token, password_hash).await? else {
        return auth_reset_or_notice(cx, None, None);
    };

    let _ = session::stop(cx).await;
    audit(cx, pool, Some(user_id), "password_reset").await;
    Err(see_other("/signin?notice=password-reset").into())
}

#[page("/account")]
pub async fn account(cx: &Cx, query: Option<Form<AccountQuery>>) -> Result<impl View> {
    let user = require_user(cx).await?;
    let notice = account_notice(
        query
            .and_then(|Form(query)| query.notice)
            .as_deref(),
    );

    account_view(cx, user, notice, None)
}

#[page(POST "/account/password")]
pub async fn change_password(
    cx: &Cx,
    Form(input): Form<ChangePasswordForm>,
) -> Result<impl View> {
    let user = require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    let pool = app_context::<PgPool>(cx);
    if !rate_limit_allows(
        pool,
        &format!("password-change:user:{}", user.id),
        10,
        3600,
    )
    .await?
    {
        return account_view(
            cx,
            user,
            None,
            Some("Too many password changes. Try again later.".to_string()),
        );
    }

    if input.new_password != input.confirm_password {
        return account_view(
            cx,
            user,
            None,
            Some("Those new passwords do not match.".to_string()),
        );
    }

    if let Err(message) = validate_password(&user.email, &input.new_password) {
        return account_view(cx, user, None, Some(message.to_string()));
    }

    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_optional(pool)
        .await?;
    let Some((current_hash,)) = row else {
        return Err(see_other("/signin").into());
    };

    if !verify_password_async(input.current_password, current_hash).await {
        return account_view(
            cx,
            user,
            None,
            Some("Your current password is incorrect.".to_string()),
        );
    }

    let new_hash = hash_password_async(input.new_password).await?;
    sqlx::query("UPDATE users SET password_hash = $2, updated_at = $3 WHERE id = $1")
        .bind(user.id)
        .bind(new_hash)
        .bind(now_seconds())
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user.id)
        .execute(pool)
        .await?;
    start_session(cx, user.id).await?;
    audit(cx, pool, Some(user.id), "password_changed").await;

    Err(see_other("/account?notice=password-changed").into())
}

#[page(POST "/account/email")]
pub async fn change_email(cx: &Cx, Form(input): Form<ChangeEmailForm>) -> Result<impl View> {
    let user = require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    let pool = app_context::<PgPool>(cx);
    if !rate_limit_allows(pool, &format!("email-change:user:{}", user.id), 5, 3600).await? {
        return account_view(
            cx,
            user,
            None,
            Some("Too many email change requests. Try again later.".to_string()),
        );
    }

    let new_email = match normalize_email(&input.new_email) {
        Ok(email) => email,
        Err(message) => return account_view(cx, user, None, Some(message.to_string())),
    };

    if new_email == user.email.to_lowercase() {
        return account_view(
            cx,
            user,
            None,
            Some("That is already your email address.".to_string()),
        );
    }

    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE email_normalized = $1 AND id <> $2)",
    )
    .bind(&new_email)
    .bind(user.id)
    .fetch_one(pool)
    .await?;

    if taken {
        return account_view(
            cx,
            user,
            None,
            Some("That email address is already in use.".to_string()),
        );
    }

    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_optional(pool)
        .await?;
    let Some((current_hash,)) = row else {
        return Err(see_other("/signin").into());
    };

    if !verify_password_async(input.current_password, current_hash).await {
        return account_view(
            cx,
            user,
            None,
            Some("Your password is incorrect.".to_string()),
        );
    }

    if let Err(error) = send_email_change_link(cx, user.id, &new_email).await {
        eprintln!("[auth] could not send email change link: {error}");
        return account_view(
            cx,
            user,
            None,
            Some("Could not send the confirmation email. Try again shortly.".to_string()),
        );
    }

    audit(cx, pool, Some(user.id), "email_change_requested").await;
    account_view(
        cx,
        user,
        Some("Confirmation sent. Check the new inbox to finish the change.".to_string()),
        None,
    )
}

#[page("/verify-email-change")]
pub async fn verify_email_change(
    cx: &Cx,
    query: Option<Form<TokenQuery>>,
) -> Result<impl View> {
    let token = query
        .and_then(|Form(query)| query.token)
        .unwrap_or_default();

    match consume_email_change_token(cx, &token).await? {
        EmailChangeOutcome::Changed(user_id) => {
            let pool = app_context::<PgPool>(cx);
            let _ = session::stop(cx).await;
            audit(cx, pool, Some(user_id), "email_changed").await;
            auth_notice(
                cx,
                "Email changed",
                "Your account email is updated. Sign in again with the new address.",
                "/signin?notice=email-changed",
                "Sign in",
            )
        }
        EmailChangeOutcome::Taken => auth_notice(
            cx,
            "Email unavailable",
            "That address is already in use. Sign in and choose a different one.",
            "/account",
            "Go to account",
        ),
        EmailChangeOutcome::Invalid => auth_notice(
            cx,
            "This link has expired",
            "That email change link is invalid or has already been used.",
            "/signin",
            "Go to sign in",
        ),
    }
}

#[page(POST "/account/delete")]
pub async fn delete_account(
    cx: &Cx,
    Form(input): Form<DeleteAccountForm>,
) -> Result<impl View> {
    let user = require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    let pool = app_context::<PgPool>(cx);
    if !rate_limit_allows(pool, &format!("delete:user:{}", user.id), 5, 3600).await? {
        return account_view(
            cx,
            user,
            None,
            Some("Too many deletion attempts. Try again later.".to_string()),
        );
    }

    if input.confirmation.trim() != "DELETE" {
        return account_view(
            cx,
            user,
            None,
            Some("Type DELETE to confirm account deletion.".to_string()),
        );
    }

    let row: Option<(String,)> = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_optional(pool)
        .await?;
    let Some((current_hash,)) = row else {
        return Err(see_other("/signin").into());
    };

    if !verify_password_async(input.password, current_hash).await {
        return account_view(
            cx,
            user,
            None,
            Some("Your password is incorrect.".to_string()),
        );
    }

    let subscription: Option<(String, String)> = sqlx::query_as(
        "SELECT dodo_subscription_id, status FROM subscriptions WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_optional(pool)
    .await?;

    if let Some((subscription_id, status)) = subscription {
        let needs_cancellation = matches!(
            status.as_str(),
            "active" | "past_due" | "on_hold" | "paused"
        );

        if needs_cancellation {
            if dodo::mode() == BillingMode::Mock {
                // Mock subscriptions are local-only.
            } else {
                let Some(config) = dodo::config() else {
                    return account_view(
                        cx,
                        user,
                        None,
                        Some(
                            "Billing is not configured. Cancel your subscription before deleting the account."
                                .to_string(),
                        ),
                    );
                };

                if let Err(error) = dodo::cancel_subscription(&config, &subscription_id).await {
                    eprintln!("[auth] subscription cancellation failed: {error}");
                    return account_view(
                        cx,
                        user,
                        None,
                        Some(
                            "Could not cancel your subscription. Open billing and cancel it first."
                                .to_string(),
                        ),
                    );
                }
            }

            audit(cx, pool, Some(user.id), "subscription_cancelled").await;
        }
    }

    let storage = app_context::<crate::sync_storage::SyncStorage>(cx);
    crate::sync::purge_user_objects(pool, storage, user.id).await;

    audit(cx, pool, Some(user.id), "account_deleted").await;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.id)
        .execute(pool)
        .await?;
    let _ = session::stop(cx).await;

    Err(see_other("/").into())
}

#[route(POST "/signout")]
pub async fn signout(cx: &Cx, Form(input): Form<CsrfForm>) -> Result<SeeOther> {
    let user = require_user(cx).await?;

    if !csrf_matches(&user.csrf_token, &input.csrf_token) {
        return Err(forbidden().into());
    }

    let pool = app_context::<PgPool>(cx);
    if let Some(token_hash) = session::stop(cx).await? {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash.to_vec())
            .execute(pool)
            .await?;
    }
    audit(cx, pool, Some(user.id), "signout").await;

    Ok(see_other("/"))
}

async fn start_session(cx: &Cx, user_id: i64) -> Result<()> {
    let pool = app_context::<PgPool>(cx);

    if let Some(previous) = session::token_hash(cx).await? {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(previous.to_vec())
            .execute(pool)
            .await?;
    }

    let session = session::start(cx).await?;
    let expires_at = session
        .expires_at
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_else(|_| now_seconds() + SESSION_LIFETIME.as_secs() as i64);
    let csrf_token = random_token();

    sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(now_seconds())
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at, csrf_token)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(session.token_hash.to_vec())
    .bind(user_id)
    .bind(expires_at)
    .bind(csrf_token)
    .execute(pool)
    .await?;

    Ok(())
}

async fn send_verification_email(cx: &Cx, user_id: i64, email: &str) -> Result<()> {
    let token = create_auth_token(cx, user_id, "verify_email", VERIFICATION_LIFETIME_SECS).await?;
    let link = format!("{}/verify-email?token={token}", base_url());
    mail::send_verification(email, &link).await
}

async fn send_email_change_link(cx: &Cx, user_id: i64, new_email: &str) -> Result<()> {
    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let token = random_token();
    let hash = token_hash(&token);

    sqlx::query(
        "UPDATE email_change_tokens SET consumed_at = $2
         WHERE user_id = $1 AND consumed_at IS NULL",
    )
    .bind(user_id)
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM email_change_tokens WHERE expires_at <= $1")
        .bind(now)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO email_change_tokens
             (user_id, token_hash, new_email, new_email_normalized, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(hash)
    .bind(new_email)
    .bind(new_email)
    .bind(now + VERIFICATION_LIFETIME_SECS)
    .execute(pool)
    .await?;

    let link = format!("{}/verify-email-change?token={token}", base_url());
    mail::send_email_change(new_email, &link).await
}

enum EmailChangeOutcome {
    Changed(i64),
    Taken,
    Invalid,
}

async fn consume_email_change_token(cx: &Cx, token: &str) -> Result<EmailChangeOutcome> {
    if token.is_empty() {
        return Ok(EmailChangeOutcome::Invalid);
    }

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, user_id, new_email, new_email_normalized
         FROM email_change_tokens
         WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > $2
         FOR UPDATE",
    )
    .bind(token_hash(token))
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((token_id, user_id, new_email, new_email_normalized)) = row else {
        return Ok(EmailChangeOutcome::Invalid);
    };

    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM users WHERE email_normalized = $1 AND id <> $2)",
    )
    .bind(&new_email_normalized)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    if taken {
        sqlx::query("UPDATE email_change_tokens SET consumed_at = $2 WHERE id = $1")
            .bind(token_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(EmailChangeOutcome::Taken);
    }

    sqlx::query(
        "UPDATE users
         SET email = $2, email_normalized = $3,
             email_verified_at = $4, updated_at = $4
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(new_email)
    .bind(new_email_normalized)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE email_change_tokens SET consumed_at = $2 WHERE id = $1")
        .bind(token_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(EmailChangeOutcome::Changed(user_id))
}

async fn send_reset_email(cx: &Cx, user_id: i64, email: &str) -> Result<()> {
    let token = create_auth_token(cx, user_id, "reset_password", RESET_LIFETIME_SECS).await?;
    let link = format!("{}/reset-password?token={token}", base_url());
    mail::send_password_reset(email, &link).await
}

async fn create_auth_token(
    cx: &Cx,
    user_id: i64,
    kind: &str,
    lifetime_secs: i64,
) -> Result<String> {
    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let token = random_token();
    let hash = token_hash(&token);

    sqlx::query(
        "UPDATE auth_tokens SET consumed_at = $2
         WHERE user_id = $1 AND kind = $3 AND consumed_at IS NULL",
    )
    .bind(user_id)
    .bind(now)
    .bind(kind)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM auth_tokens WHERE expires_at <= $1")
        .bind(now)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO auth_tokens (user_id, kind, token_hash, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(kind)
    .bind(hash)
    .bind(now + lifetime_secs)
    .execute(pool)
    .await?;

    Ok(token)
}

async fn consume_verification_token(cx: &Cx, token: &str) -> Result<Option<i64>> {
    if token.is_empty() {
        return Ok(None);
    }

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM auth_tokens
         WHERE token_hash = $1 AND kind = 'verify_email'
           AND consumed_at IS NULL AND expires_at > $2
         FOR UPDATE",
    )
    .bind(token_hash(token))
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((token_id, user_id)) = row else {
        return Ok(None);
    };

    sqlx::query("UPDATE auth_tokens SET consumed_at = $2 WHERE id = $1")
        .bind(token_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE users SET email_verified_at = COALESCE(email_verified_at, $2), updated_at = $2
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Some(user_id))
}

async fn auth_token_is_valid(cx: &Cx, token: &str, kind: &str) -> Result<bool> {
    if token.is_empty() {
        return Ok(false);
    }

    let pool = app_context::<PgPool>(cx);
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM auth_tokens
            WHERE token_hash = $1 AND kind = $2
              AND consumed_at IS NULL AND expires_at > $3
         )",
    )
    .bind(token_hash(token))
    .bind(kind)
    .bind(now_seconds())
    .fetch_one(pool)
    .await?;

    Ok(valid)
}

async fn replace_password_with_token(
    cx: &Cx,
    token: &str,
    password_hash: String,
) -> Result<Option<i64>> {
    if token.is_empty() {
        return Ok(None);
    }

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let mut tx = pool.begin().await?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM auth_tokens
         WHERE token_hash = $1 AND kind = 'reset_password'
           AND consumed_at IS NULL AND expires_at > $2
         FOR UPDATE",
    )
    .bind(token_hash(token))
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((token_id, user_id)) = row else {
        return Ok(None);
    };

    sqlx::query("UPDATE users SET password_hash = $2, updated_at = $3 WHERE id = $1")
        .bind(user_id)
        .bind(password_hash)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE auth_tokens SET consumed_at = $2 WHERE id = $1")
        .bind(token_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(Some(user_id))
}

pub(crate) async fn rate_limit_allows(
    pool: &PgPool,
    key: &str,
    limit: i32,
    window_secs: i64,
) -> Result<bool> {
    let window_start = now_seconds() / window_secs * window_secs;
    let count: i32 = sqlx::query_scalar(
        "INSERT INTO rate_limits (key, window_start, count)
         VALUES ($1, $2, 1)
         ON CONFLICT (key) DO UPDATE SET
             count = CASE
                 WHEN rate_limits.window_start = EXCLUDED.window_start
                 THEN rate_limits.count + 1
                 ELSE 1
             END,
             window_start = EXCLUDED.window_start
         RETURNING count",
    )
    .bind(key)
    .bind(window_start)
    .fetch_one(pool)
    .await?;

    Ok(count <= limit)
}

pub(crate) async fn audit(cx: &Cx, pool: &PgPool, user_id: Option<i64>, action: &str) {
    let ip = client_ip(cx).map(|ip| ip.to_string());
    let user_agent = headers(cx)
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.chars().take(512).collect::<String>());

    if let Err(error) = sqlx::query(
        "INSERT INTO audit_log (user_id, action, ip, user_agent)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(action)
    .bind(ip)
    .bind(user_agent)
    .execute(pool)
    .await
    {
        eprintln!("[auth] audit log write failed: {error}");
    }
}

fn client_ip_key(cx: &Cx) -> String {
    client_ip(cx)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn csrf_matches(expected: &str, provided: &str) -> bool {
    let expected = expected.as_bytes();
    let provided = provided.as_bytes();

    if expected.len() != provided.len() {
        return false;
    }

    let mut difference = 0_u8;
    for (left, right) in expected.iter().zip(provided) {
        difference |= left ^ right;
    }
    difference == 0
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).expect("the system random number generator failed");
    URL_SAFE_NO_PAD.encode(bytes)
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn base_url() -> String {
    std::env::var("APP_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn account_notice(key: Option<&str>) -> Option<String> {
    match key {
        Some("verification-sent") => {
            Some("Verification email sent. Check your inbox.".to_string())
        }
        Some("email-verified") => Some("Your email address is verified.".to_string()),
        Some("password-changed") => {
            Some("Password changed. Other sessions were signed out.".to_string())
        }
        Some("email-change-sent") => {
            Some("Confirmation sent. Check the new inbox to finish the change.".to_string())
        }
        _ => None,
    }
}

fn signin_notice(key: Option<&str>) -> Option<String> {
    match key {
        Some("password-reset") => {
            Some("Password updated. Sign in with your new password.".to_string())
        }
        Some("email-changed") => {
            Some("Email updated. Sign in with your new address.".to_string())
        }
        _ => None,
    }
}

fn normalize_plan(plan: Option<&str>) -> Result<&'static str, &'static str> {
    match plan.unwrap_or("monthly") {
        "monthly" => Ok("monthly"),
        "yearly" => Ok("yearly"),
        _ => Err("Choose a monthly or yearly plan."),
    }
}

fn normalize_email(raw: &str) -> Result<String, &'static str> {
    let email = raw.trim().to_lowercase();

    if email.len() < 3 || email.len() > 254 {
        return Err("Enter a valid email address.");
    }

    if email
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("Enter a valid email address.");
    }

    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();

    if parts.next().is_some()
        || local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || domain.contains("..")
        || !domain.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '.'
        })
    {
        return Err("Enter a valid email address.");
    }

    Ok(email)
}

fn validate_password(email: &str, password: &str) -> Result<(), &'static str> {
    let length = password.chars().count();

    if !(MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(&length) {
        return Err("Use a password between 12 and 128 characters.");
    }

    if password.trim().is_empty() {
        return Err("Use a password with at least one visible character.");
    }

    if !email.is_empty() && password.eq_ignore_ascii_case(email) {
        return Err("Choose a password that is not your email address.");
    }

    let lower = password.to_lowercase();
    if COMMON_PASSWORDS.iter().any(|common| *common == lower) {
        return Err("Choose a less common password.");
    }

    Ok(())
}

fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(HASH_MEMORY_KIB, HASH_ITERATIONS, HASH_PARALLELISM, None)
            .expect("the Argon2 parameters are valid"),
    )
}

fn hash_password(password: &str) -> Result<String> {
    Ok(argon2().hash_password(password.as_bytes())?.to_string())
}

fn verify_password(password: &str, hash: &str) -> bool {
    argon2().verify_password(password.as_bytes(), hash).is_ok()
}

async fn hash_password_async(password: String) -> Result<String> {
    Ok(tokio::task::spawn_blocking(move || hash_password(&password)).await??)
}

async fn verify_password_async(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || verify_password(&password, &hash))
        .await
        .unwrap_or(false)
}

async fn verify_dummy_password(password: String) {
    let dummy = DUMMY_HASH
        .get_or_init(|| {
            hash_password("a dummy password used only to equalize sign-in timing")
                .unwrap_or_default()
        })
        .clone();

    if !dummy.is_empty() {
        let _ = verify_password_async(password, dummy).await;
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn auth_signup(
    cx: &Cx,
    error: Option<String>,
    email: String,
    plan: String,
) -> Result<impl View> {
    Ok(view! {
        cx =>
        <main
            class="flex min-h-screen items-center justify-center bg-surface px-5 py-10"
        >
            <div class="w-full max-w-sm">
                <a href="/" class="inline-flex items-center gap-3">
                    <span
                        class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                    >
                        "D"
                    </span>
                    <span
                        class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                    >
                        "Dainiki"
                    </span>
                </a>

                <div
                    class="mt-8 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Sync account"
                    </p>
                    <h1
                        class="mt-2 font-display text-3xl leading-tight text-on-surface"
                    >
                        "Create an account"
                    </h1>
                    <p class="mt-2 font-ui text-ui-body text-on-surface-variant">
                        "Accounts are only for paid, encrypted sync. Your journal stays on this device."
                    </p>

                    if let Some(message) = error {
                        <p
                            role="alert"
                            class="mt-4 rounded border border-secondary/40 bg-secondary/10 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    <form method="post" action="/signup" class="mt-5 space-y-4">
                        <label class="block">
                            <span class="font-ui text-ui-label text-on-surface-variant">
                                "Email"
                            </span>
                            <input
                                id="signup-email"
                                name="email"
                                type="email"
                                autocomplete="email"
                                required="true"
                                value=(email)
                                class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                            >
                        </label>

                        <label class="block">
                            <span class="font-ui text-ui-label text-on-surface-variant">
                                "Password"
                            </span>
                            <input
                                id="signup-password"
                                name="password"
                                type="password"
                                autocomplete="new-password"
                                minlength="12"
                                maxlength="128"
                                required="true"
                                class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                            >
                            <span
                                class="mt-1 block font-ui text-[10px] text-on-surface-variant/70"
                            >
                                "At least 12 characters."
                            </span>
                        </label>

                        <label class="block">
                            <span class="font-ui text-ui-label text-on-surface-variant">
                                "Confirm password"
                            </span>
                            <input
                                id="signup-confirm"
                                name="confirm_password"
                                type="password"
                                autocomplete="new-password"
                                minlength="12"
                                maxlength="128"
                                required="true"
                                class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                            >
                        </label>

                        <fieldset>
                            <legend
                                class="font-ui text-ui-label text-on-surface-variant"
                            >
                                "Plan"
                            </legend>
                            <div class="mt-2 grid gap-2 sm:grid-cols-2">
                                <label
                                    class="flex cursor-pointer items-start gap-2 rounded border border-outline-variant/60 px-3 py-2 font-ui text-ui-body text-on-surface has-[:checked]:border-primary has-[:checked]:bg-primary/5"
                                >
                                    <input
                                        type="radio"
                                        name="plan"
                                        value="monthly"
                                        checked=(plan == "monthly")
                                        class="mt-1 accent-primary"
                                    >
                                    <span>
                                        <span class="block font-medium">"Monthly"</span>
                                        <span class="block text-ui-label text-on-surface-variant">
                                            "$2 / month"
                                        </span>
                                    </span>
                                </label>
                                <label
                                    class="flex cursor-pointer items-start gap-2 rounded border border-outline-variant/60 px-3 py-2 font-ui text-ui-body text-on-surface has-[:checked]:border-primary has-[:checked]:bg-primary/5"
                                >
                                    <input
                                        type="radio"
                                        name="plan"
                                        value="yearly"
                                        checked=(plan == "yearly")
                                        class="mt-1 accent-primary"
                                    >
                                    <span>
                                        <span class="block font-medium">"Yearly"</span>
                                        <span class="block text-ui-label text-on-surface-variant">
                                            "$20 / year"
                                        </span>
                                    </span>
                                </label>
                            </div>
                        </fieldset>

                        <button
                            id="signup-submit"
                            type="submit"
                            class="w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                        >
                            "Create account"
                        </button>
                    </form>

                    <p
                        class="mt-5 text-center font-ui text-ui-label text-on-surface-variant"
                    >
                        "Already have an account? "
                        <a href="/signin" class="text-primary hover:underline">
                            "Sign in"
                        </a>
                    </p>
                </div>

                <p
                    class="mt-6 text-center font-ui text-[10px] text-on-surface-variant/60"
                >
                    "No account needed for local journaling."
                </p>
            </div>
        </main>
    })
}

fn auth_signin(
    cx: &Cx,
    error: Option<String>,
    notice: Option<String>,
    email: String,
) -> Result<impl View> {
    Ok(view! {
        cx =>
        <main
            class="flex min-h-screen items-center justify-center bg-surface px-5 py-10"
        >
            <div class="w-full max-w-sm">
                <a href="/" class="inline-flex items-center gap-3">
                    <span
                        class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                    >
                        "D"
                    </span>
                    <span
                        class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                    >
                        "Dainiki"
                    </span>
                </a>

                <div
                    class="mt-8 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Sync account"
                    </p>
                    <h1
                        class="mt-2 font-display text-3xl leading-tight text-on-surface"
                    >
                        "Sign in"
                    </h1>
                    <p class="mt-2 font-ui text-ui-body text-on-surface-variant">
                        "Sign in to manage your sync account. Local journaling works without an account."
                    </p>

                    if let Some(message) = notice {
                        <p
                            class="mt-4 rounded border border-primary/30 bg-primary/5 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    if let Some(message) = error {
                        <p
                            role="alert"
                            class="mt-4 rounded border border-secondary/40 bg-secondary/10 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    <form method="post" action="/signin" class="mt-5 space-y-4">
                        <label class="block">
                            <span class="font-ui text-ui-label text-on-surface-variant">
                                "Email"
                            </span>
                            <input
                                id="signin-email"
                                name="email"
                                type="email"
                                autocomplete="email"
                                required="true"
                                value=(email)
                                class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                            >
                        </label>

                        <label class="block">
                            <span class="font-ui text-ui-label text-on-surface-variant">
                                "Password"
                            </span>
                            <input
                                id="signin-password"
                                name="password"
                                type="password"
                                autocomplete="current-password"
                                required="true"
                                class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                            >
                        </label>

                        <div class="flex justify-end">
                            <a
                                href="/forgot-password"
                                class="font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
                            >
                                "Forgot password?"
                            </a>
                        </div>

                        <button
                            id="signin-submit"
                            type="submit"
                            class="w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                        >
                            "Sign in"
                        </button>
                    </form>

                    <p
                        class="mt-5 text-center font-ui text-ui-label text-on-surface-variant"
                    >
                        "New here? "
                        <a href="/signup" class="text-primary hover:underline">
                            "Create a sync account"
                        </a>
                    </p>
                </div>

                <p
                    class="mt-6 text-center font-ui text-[10px] text-on-surface-variant/60"
                >
                    "No account needed for local journaling."
                </p>
            </div>
        </main>
    })
}

fn auth_forgot(cx: &Cx, sent: bool, email: String) -> Result<impl View> {
    Ok(view! {
        cx =>
        <main
            class="flex min-h-screen items-center justify-center bg-surface px-5 py-10"
        >
            <div class="w-full max-w-sm">
                <a href="/" class="inline-flex items-center gap-3">
                    <span
                        class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                    >
                        "D"
                    </span>
                    <span
                        class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                    >
                        "Dainiki"
                    </span>
                </a>

                <div
                    class="mt-8 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Account recovery"
                    </p>
                    <h1
                        class="mt-2 font-display text-3xl leading-tight text-on-surface"
                    >
                        "Reset your password"
                    </h1>

                    if sent {
                        <p
                            class="mt-4 rounded border border-primary/30 bg-primary/5 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            "If an account exists for that email, a reset link is on its way. Check your inbox."
                        </p>
                    } else {
                        <p class="mt-2 font-ui text-ui-body text-on-surface-variant">
                            "Enter the email address for your sync account. We'll send a link to choose a new password."
                        </p>
                    }

                    <form
                        method="post"
                        action="/forgot-password"
                        class="mt-5 space-y-4"
                    >
                        <label class="block">
                            <span class="font-ui text-ui-label text-on-surface-variant">
                                "Email"
                            </span>
                            <input
                                id="forgot-email"
                                name="email"
                                type="email"
                                autocomplete="email"
                                required="true"
                                value=(email)
                                class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                            >
                        </label>

                        <button
                            id="forgot-submit"
                            type="submit"
                            class="w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                        >
                            "Send reset link"
                        </button>
                    </form>

                    <p
                        class="mt-5 text-center font-ui text-ui-label text-on-surface-variant"
                    >
                        <a href="/signin" class="text-primary hover:underline">
                            "Back to sign in"
                        </a>
                    </p>
                </div>
            </div>
        </main>
    })
}

fn auth_reset_or_notice(
    cx: &Cx,
    token: Option<String>,
    error: Option<String>,
) -> Result<impl View> {
    Ok(view! {
        cx =>
        <main
            class="flex min-h-screen items-center justify-center bg-surface px-5 py-10"
        >
            <div class="w-full max-w-sm">
                <a href="/" class="inline-flex items-center gap-3">
                    <span
                        class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                    >
                        "D"
                    </span>
                    <span
                        class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                    >
                        "Dainiki"
                    </span>
                </a>

                <div
                    class="mt-8 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Account recovery"
                    </p>

                    if let Some(token) = token {
                        <h1
                            class="mt-2 font-display text-3xl leading-tight text-on-surface"
                        >
                            "Choose a new password"
                        </h1>
                        <p class="mt-2 font-ui text-ui-body text-on-surface-variant">
                            "Other sessions will be signed out after the change."
                        </p>

                        if let Some(message) = error {
                            <p
                                role="alert"
                                class="mt-4 rounded border border-secondary/40 bg-secondary/10 px-3 py-2 font-ui text-ui-label text-on-surface"
                            >
                                (message)
                            </p>
                        }

                        <form
                            method="post"
                            action="/reset-password"
                            class="mt-5 space-y-4"
                        >
                            <input type="hidden" name="token" value=(token)>
                            <label class="block">
                                <span
                                    class="font-ui text-ui-label text-on-surface-variant"
                                >
                                    "New password"
                                </span>
                                <input
                                    id="reset-password"
                                    name="password"
                                    type="password"
                                    autocomplete="new-password"
                                    minlength="12"
                                    maxlength="128"
                                    required="true"
                                    class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                >
                            </label>

                            <label class="block">
                                <span
                                    class="font-ui text-ui-label text-on-surface-variant"
                                >
                                    "Confirm password"
                                </span>
                                <input
                                    id="reset-confirm"
                                    name="confirm_password"
                                    type="password"
                                    autocomplete="new-password"
                                    minlength="12"
                                    maxlength="128"
                                    required="true"
                                    class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                >
                            </label>

                            <button
                                id="reset-submit"
                                type="submit"
                                class="w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                            >
                                "Update password"
                            </button>
                        </form>
                    } else {
                        <h1
                            class="mt-2 font-display text-3xl leading-tight text-on-surface"
                        >
                            "Reset link invalid"
                        </h1>
                        <p class="mt-3 font-ui text-ui-body text-on-surface-variant">
                            "That password reset link is invalid or has expired. Request a new one to continue."
                        </p>
                        <a
                            href="/forgot-password"
                            class="mt-5 inline-flex rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                        >
                            "Request a new link"
                        </a>
                    }
                </div>
            </div>
        </main>
    })
}

fn auth_notice(
    cx: &Cx,
    title: &str,
    message: &str,
    action_href: &str,
    action_label: &str,
) -> Result<impl View> {
    Ok(view! {
        cx =>
        <main
            class="flex min-h-screen items-center justify-center bg-surface px-5 py-10"
        >
            <div class="w-full max-w-sm">
                <a href="/" class="inline-flex items-center gap-3">
                    <span
                        class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                    >
                        "D"
                    </span>
                    <span
                        class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                    >
                        "Dainiki"
                    </span>
                </a>

                <div
                    class="mt-8 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Sync account"
                    </p>
                    <h1
                        class="mt-2 font-display text-3xl leading-tight text-on-surface"
                    >
                        (title)
                    </h1>
                    <p class="mt-3 font-ui text-ui-body text-on-surface-variant">
                        (message)
                    </p>
                    <a
                        href=(action_href)
                        class="mt-5 inline-flex rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                    >
                        (action_label)
                    </a>
                </div>
            </div>
        </main>
    })
}

fn account_view(
    cx: &Cx,
    user: CurrentUser,
    notice: Option<String>,
    error: Option<String>,
) -> Result<impl View> {
    let plan = match user.plan.as_deref() {
        Some("monthly") => "Monthly · $2 / month".to_string(),
        Some("yearly") => "Yearly · $20 / year".to_string(),
        Some(other) => other.to_string(),
        None => "No plan selected".to_string(),
    };
    let status = if user.verified {
        "Email verified"
    } else {
        "Email not verified"
    };
    let user_id = user.id;
    let csrf_token = user.csrf_token;
    let email = user.email;
    let verified = user.verified;

    Ok(view! {
        cx =>
        <main class="min-h-screen bg-surface px-5 py-10">
            <div class="mx-auto w-full max-w-2xl">
                <header class="flex items-center justify-between gap-4">
                    <a href="/" class="inline-flex items-center gap-3">
                        <span
                            class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                        >
                            "D"
                        </span>
                        <span
                            class="font-ui text-sm font-semibold tracking-tight text-on-surface"
                        >
                            "Dainiki"
                        </span>
                    </a>
                    <a
                        href="/"
                        class="font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
                    >
                        "Back to journal"
                    </a>
                </header>

                <div
                    class="mt-12 rounded-xl border border-outline-variant/40 bg-surface-container-lowest p-6 shadow-sm sm:p-8"
                >
                    <p
                        class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                    >
                        "Sync account"
                    </p>
                    <h1
                        class="mt-2 font-display text-3xl leading-tight text-on-surface"
                    >
                        "Your account"
                    </h1>
                    <p class="mt-2 font-ui text-ui-body text-on-surface-variant">
                        "Your local journal is separate from this account and always stays on this device."
                    </p>

                    if let Some(message) = notice {
                        <p
                            class="mt-4 rounded border border-primary/30 bg-primary/5 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    if let Some(message) = error {
                        <p
                            role="alert"
                            class="mt-4 rounded border border-secondary/40 bg-secondary/10 px-3 py-2 font-ui text-ui-label text-on-surface"
                        >
                            (message)
                        </p>
                    }

                    <dl
                        class="mt-7 divide-y divide-outline-variant/30 border-y border-outline-variant/30"
                    >
                        <div class="flex items-start justify-between gap-4 py-3">
                            <dt class="font-ui text-ui-label text-on-surface-variant">
                                "Email"
                            </dt>
                            <dd
                                class="break-all text-right font-ui text-ui-body text-on-surface"
                            >
                                (email)
                            </dd>
                        </div>
                        <div class="flex items-start justify-between gap-4 py-3">
                            <dt class="font-ui text-ui-label text-on-surface-variant">
                                "Status"
                            </dt>
                            <dd class="text-right font-ui text-ui-body text-on-surface">
                                (status)
                                if !verified {
                                    <form
                                        method="post"
                                        action="/verify-email/resend"
                                        class="mt-1"
                                    >
                                        <input
                                            type="hidden"
                                            name="csrf_token"
                                            value=(csrf_token.clone())
                                        >
                                        <button
                                            id="resend-verification"
                                            type="submit"
                                            class="font-ui text-ui-label text-primary hover:underline"
                                        >
                                            "Resend verification"
                                        </button>
                                    </form>
                                }
                            </dd>
                        </div>
                        <div class="flex items-start justify-between gap-4 py-3">
                            <dt class="font-ui text-ui-label text-on-surface-variant">
                                "Plan"
                            </dt>
                            <dd class="text-right font-ui text-ui-body text-on-surface">
                                (plan)
                            </dd>
                        </div>
                    </dl>

                    billing_section(user_id: user_id, csrf_token: csrf_token.clone())
                    <section class="mt-8 border-t border-outline-variant/30 pt-6">
                        <h2 class="font-display text-xl text-on-surface">
                            "Email address"
                        </h2>
                        <p class="mt-1 font-ui text-ui-body text-on-surface-variant">
                            "We'll send a confirmation link to the new address. Your current email stays active until you confirm."
                        </p>
                        <form
                            method="post"
                            action="/account/email"
                            class="mt-4 space-y-3"
                        >
                            <input
                                type="hidden"
                                name="csrf_token"
                                value=(csrf_token.clone())
                            >
                            <label class="block">
                                <span
                                    class="font-ui text-ui-label text-on-surface-variant"
                                >
                                    "New email"
                                </span>
                                <input
                                    id="new-email"
                                    name="new_email"
                                    type="email"
                                    autocomplete="email"
                                    required="true"
                                    class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                >
                            </label>
                            <label class="block">
                                <span
                                    class="font-ui text-ui-label text-on-surface-variant"
                                >
                                    "Current password"
                                </span>
                                <input
                                    id="email-password"
                                    name="current_password"
                                    type="password"
                                    autocomplete="current-password"
                                    required="true"
                                    class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                >
                            </label>
                            <button
                                id="change-email"
                                type="submit"
                                class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                            >
                                "Send confirmation"
                            </button>
                        </form>
                    </section>

                    <section class="mt-8 border-t border-outline-variant/30 pt-6">
                        <h2 class="font-display text-xl text-on-surface">"Password"</h2>
                        <p class="mt-1 font-ui text-ui-body text-on-surface-variant">
                            "Changing your password signs out other sessions."
                        </p>
                        <form
                            method="post"
                            action="/account/password"
                            class="mt-4 space-y-3"
                        >
                            <input
                                type="hidden"
                                name="csrf_token"
                                value=(csrf_token.clone())
                            >
                            <label class="block">
                                <span
                                    class="font-ui text-ui-label text-on-surface-variant"
                                >
                                    "Current password"
                                </span>
                                <input
                                    id="current-password"
                                    name="current_password"
                                    type="password"
                                    autocomplete="current-password"
                                    required="true"
                                    class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                >
                            </label>
                            <div class="grid gap-3 sm:grid-cols-2">
                                <label class="block">
                                    <span
                                        class="font-ui text-ui-label text-on-surface-variant"
                                    >
                                        "New password"
                                    </span>
                                    <input
                                        id="new-password"
                                        name="new_password"
                                        type="password"
                                        autocomplete="new-password"
                                        minlength="12"
                                        maxlength="128"
                                        required="true"
                                        class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                    >
                                </label>
                                <label class="block">
                                    <span
                                        class="font-ui text-ui-label text-on-surface-variant"
                                    >
                                        "Confirm new password"
                                    </span>
                                    <input
                                        id="new-password-confirm"
                                        name="confirm_password"
                                        type="password"
                                        autocomplete="new-password"
                                        minlength="12"
                                        maxlength="128"
                                        required="true"
                                        class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                    >
                                </label>
                            </div>
                            <button
                                id="change-password"
                                type="submit"
                                class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                            >
                                "Change password"
                            </button>
                        </form>
                    </section>

                    <section class="mt-8 border-t border-outline-variant/30 pt-6">
                        <h2 class="font-display text-xl text-on-surface">"Session"</h2>
                        <p class="mt-1 font-ui text-ui-body text-on-surface-variant">
                            "Signing out only ends this account session. Your local journal is not deleted."
                        </p>
                        <form method="post" action="/signout" class="mt-4">
                            <input
                                type="hidden"
                                name="csrf_token"
                                value=(csrf_token.clone())
                            >
                            <button
                                id="signout"
                                type="submit"
                                class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface hover:bg-surface-container"
                            >
                                "Sign out"
                            </button>
                        </form>
                    </section>

                    <section class="mt-8 border-t border-outline-variant/30 pt-6">
                        <details>
                            <summary
                                class="cursor-pointer font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
                            >
                                "Delete account"
                            </summary>
                            <p
                                class="mt-3 font-ui text-ui-body text-on-surface-variant"
                            >
                                "This removes your sync account and its sessions. Your local journal is not touched."
                            </p>
                            <form
                                method="post"
                                action="/account/delete"
                                class="mt-4 space-y-3"
                            >
                                <input
                                    type="hidden"
                                    name="csrf_token"
                                    value=(csrf_token.clone())
                                >
                                <label class="block">
                                    <span
                                        class="font-ui text-ui-label text-on-surface-variant"
                                    >
                                        "Password"
                                    </span>
                                    <input
                                        id="delete-password"
                                        name="password"
                                        type="password"
                                        autocomplete="current-password"
                                        required="true"
                                        class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                    >
                                </label>
                                <label class="block">
                                    <span
                                        class="font-ui text-ui-label text-on-surface-variant"
                                    >
                                        "Type DELETE to confirm"
                                    </span>
                                    <input
                                        id="delete-confirmation"
                                        name="confirmation"
                                        type="text"
                                        autocomplete="off"
                                        required="true"
                                        class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"
                                    >
                                </label>
                                <button
                                    id="delete-account"
                                    type="submit"
                                    class="rounded border border-secondary/60 px-4 py-2 font-ui text-ui-label text-secondary hover:bg-secondary/10"
                                >
                                    "Delete account"
                                </button>
                            </form>
                        </details>
                    </section>
                </div>

                <p
                    class="mt-6 text-center font-ui text-[10px] text-on-surface-variant/60"
                >
                    "Encrypted sync is not connected on this device yet."
                </p>
            </div>
        </main>
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalization_trims_and_lowercases() {
        assert_eq!(
            normalize_email("  Person@Example.COM ").unwrap(),
            "person@example.com"
        );
    }

    #[test]
    fn email_validation_rejects_obvious_problems() {
        for email in [
            "",
            "not-an-email",
            "a@b",
            "@example.com",
            "person@",
            "person@@example.com",
            "person@example..com",
            "person name@example.com",
        ] {
            assert!(normalize_email(email).is_err(), "{email}");
        }
    }

    #[test]
    fn password_rules_are_enforced() {
        assert!(validate_password("person@example.com", "short").is_err());
        assert!(validate_password("person@example.com", "person@example.com").is_err());
        assert!(validate_password("person@example.com", "correcthorsebatterystaple").is_err());
        assert!(validate_password("person@example.com", "a long and quiet passphrase").is_ok());
    }

    #[test]
    fn hashing_round_trips() {
        let hash = hash_password("a long and quiet passphrase").unwrap();
        assert!(verify_password("a long and quiet passphrase", &hash));
        assert!(!verify_password("the wrong passphrase", &hash));
    }

    #[test]
    fn csrf_comparison_is_exact() {
        assert!(csrf_matches("abc", "abc"));
        assert!(!csrf_matches("abc", "abd"));
        assert!(!csrf_matches("abc", "ab"));
    }
}
