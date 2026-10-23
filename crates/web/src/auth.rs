use std::{
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier},
};
use serde::Deserialize;
use sqlx::PgPool;
use topcoat::{
    Result,
    context::{Cx, app_context},
    cookie::{Cookie, Cookies, SameSite, cookies, time::Duration as CookieDuration},
    router::{
        content::Form,
        error::{SeeOther, see_other},
        page, route,
    },
    session::{self, SessionConfig, Token, TokenStore, TokenStoreFuture},
    view::{View, view},
};

const SESSION_LIFETIME: Duration = Duration::from_secs(60 * 60 * 24 * 30);
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
    pub email: String,
    pub plan: Option<String>,
    pub verified: bool,
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

pub async fn current_user(cx: &Cx) -> Result<Option<CurrentUser>> {
    let Some(token_hash) = session::token_hash(cx).await? else {
        return Ok(None);
    };

    let pool = app_context::<PgPool>(cx);
    let row: Option<(String, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT u.email, u.plan, u.email_verified_at
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.expires_at > $2",
    )
    .bind(token_hash.to_vec())
    .bind(now_seconds())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(email, plan, verified_at)| CurrentUser {
        email,
        plan,
        verified: verified_at.is_some(),
    }))
}

#[page("/signup")]
pub async fn signup_page(cx: &Cx) -> Result<impl View> {
    if current_user(cx).await?.is_some() {
        return Err(see_other("/account").into());
    }

    auth_signup(cx, None, String::new(), "monthly".to_string())
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

    let password_hash = hash_password_async(input.password).await?;
    let pool = app_context::<PgPool>(cx);
    let inserted = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (email, email_normalized, password_hash, plan)
         VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(input.email.trim())
    .bind(&email)
    .bind(password_hash)
    .bind(plan)
    .fetch_one(pool)
    .await;

    match inserted {
        Ok(user_id) => {
            start_session(cx, user_id).await?;
            Err(see_other("/account").into())
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
pub async fn signin_page(cx: &Cx) -> Result<impl View> {
    if current_user(cx).await?.is_some() {
        return Err(see_other("/account").into());
    }

    auth_signin(cx, None, String::new())
}

#[page(POST "/signin")]
pub async fn signin(cx: &Cx, Form(input): Form<SigninForm>) -> Result<impl View> {
    let message = "Email or password is incorrect.";
    let email = normalize_email(&input.email);

    let Some(email) = email.ok() else {
        verify_dummy_password(input.password).await;
        return auth_signin(cx, Some(message.to_string()), input.email);
    };

    let pool = app_context::<PgPool>(cx);
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE email_normalized = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await?;

    let Some((user_id, password_hash)) = row else {
        verify_dummy_password(input.password).await;
        return auth_signin(cx, Some(message.to_string()), input.email);
    };

    if !verify_password_async(input.password, password_hash).await {
        return auth_signin(cx, Some(message.to_string()), input.email);
    }

    start_session(cx, user_id).await?;
    Err(see_other("/account").into())
}

#[page("/account")]
pub async fn account(cx: &Cx) -> Result<impl View> {
    let Some(user) = current_user(cx).await? else {
        return Err(see_other("/signin").into());
    };

    account_view(cx, user)
}

#[route(POST "/signout")]
pub async fn signout(cx: &Cx) -> Result<SeeOther> {
    let pool = app_context::<PgPool>(cx);

    if let Some(token_hash) = session::stop(cx).await? {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash.to_vec())
            .execute(pool)
            .await?;
    }

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

    sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(now_seconds())
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(session.token_hash.to_vec())
    .bind(user_id)
    .bind(expires_at)
    .execute(pool)
    .await?;

    Ok(())
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

    if password.eq_ignore_ascii_case(email) {
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

fn auth_signup(cx: &Cx, error: Option<String>, email: String, plan: String) -> Result<impl View> {
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

fn auth_signin(cx: &Cx, error: Option<String>, email: String) -> Result<impl View> {
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

fn account_view(cx: &Cx, user: CurrentUser) -> Result<impl View> {
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

                    <dl
                        class="mt-7 divide-y divide-outline-variant/30 border-y border-outline-variant/30"
                    >
                        <div class="flex items-start justify-between gap-4 py-3">
                            <dt class="font-ui text-ui-label text-on-surface-variant">
                                "Email"
                            </dt>
                            <dd class="text-right font-ui text-ui-body text-on-surface">
                                (user.email)
                            </dd>
                        </div>
                        <div class="flex items-start justify-between gap-4 py-3">
                            <dt class="font-ui text-ui-label text-on-surface-variant">
                                "Status"
                            </dt>
                            <dd class="text-right font-ui text-ui-body text-on-surface">
                                (status)
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

                    <div class="mt-7 border-t border-outline-variant/30 pt-5">
                        <p class="font-ui text-ui-body text-on-surface-variant">
                            "Signing out only ends this account session. Your local journal is not deleted."
                        </p>
                        <form method="post" action="/signout" class="mt-4">
                            <button
                                id="signout"
                                type="submit"
                                class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface hover:bg-surface-container"
                            >
                                "Sign out"
                            </button>
                        </form>
                    </div>
                </div>

                <p
                    class="mt-6 text-center font-ui text-[10px] text-on-surface-variant/60"
                >
                    "Encrypted sync is not connected yet."
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
}
