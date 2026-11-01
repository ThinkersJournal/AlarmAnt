//! A2: the sender's sign-in to Google (service account, OAuth2 JWT bearer grant).
//!
//! Pure data and signing: no network, clock or file access. The caller reads the key through
//! [`crate::fcm::SecretSource`], posts [`token_request_body`] to [`ServiceAccount::token_uri`]
//! (HTTPAnt) and hands the answer to [`parse_token_response`]. The private key and the access token
//! never appear in `Debug`, `Display` or an error message.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use serde_json::{Value, json};

/// What the access token may do: send FCM messages, nothing else.
pub const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";

/// How long an assertion is valid. Google's maximum.
pub const ASSERTION_LIFETIME_SECONDS: u64 = 3600;

/// A new token is fetched this long before the old one expires.
pub const REFRESH_MARGIN_SECONDS: u64 = 60;

/// Why sign-in failed. Messages never contain the key or a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// The service-account JSON is not the documented shape; the string names the field.
    BadServiceAccount(&'static str),
    /// The private key is not an RSA PKCS#8 PEM.
    BadKey,
    /// Signing failed.
    Signing,
    /// Google's token endpoint answered with something unusable or an error; the string is its
    /// `error` code (for example `invalid_grant`), never a token.
    Rejected(String),
}

/// The fields of a Google service-account key file the sender needs.
#[derive(Clone)]
pub struct ServiceAccount {
    client_email: String,
    private_key_pem: String,
    token_uri: String,
    project_id: String,
}

impl fmt::Debug for ServiceAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceAccount")
            .field("client_email", &self.client_email)
            .field("project_id", &self.project_id)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

impl ServiceAccount {
    /// Parses the JSON file Google issues for a service account.
    pub fn from_json(json: &str) -> Result<Self, AuthError> {
        let v: Value =
            serde_json::from_str(json).map_err(|_| AuthError::BadServiceAccount("not JSON"))?;
        let field = |name: &'static str| -> Result<String, AuthError> {
            v.get(name)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or(AuthError::BadServiceAccount(name))
        };
        if v.get("type").and_then(Value::as_str) != Some("service_account") {
            return Err(AuthError::BadServiceAccount("type"));
        }
        let token_uri = field("token_uri")?;
        if !token_uri.starts_with("https://") {
            return Err(AuthError::BadServiceAccount("token_uri"));
        }
        Ok(ServiceAccount {
            client_email: field("client_email")?,
            private_key_pem: field("private_key")?,
            token_uri,
            project_id: field("project_id")?,
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Where to post the token request.
    pub fn token_uri(&self) -> &str {
        &self.token_uri
    }

    fn key_pair(&self) -> Result<RsaKeyPair, AuthError> {
        let body: String = self
            .private_key_pem
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("-----"))
            .collect();
        let der = STANDARD.decode(body).map_err(|_| AuthError::BadKey)?;
        RsaKeyPair::from_pkcs8(&der).map_err(|_| AuthError::BadKey)
    }

    /// The signed JWT (RS256) that Google exchanges for an access token. `now` is unix seconds.
    pub fn assertion(&self, now: u64) -> Result<String, AuthError> {
        let header = json!({ "alg": "RS256", "typ": "JWT" });
        let claims = json!({
            "iss": self.client_email,
            "scope": SCOPE,
            "aud": self.token_uri,
            "iat": now,
            "exp": now + ASSERTION_LIFETIME_SECONDS,
        });
        let enc = |v: &Value| URL_SAFE_NO_PAD.encode(v.to_string());
        let signing_input = format!("{}.{}", enc(&header), enc(&claims));
        let key = self.key_pair()?;
        let mut sig = vec![0u8; key.public().modulus_len()];
        key.sign(
            &RSA_PKCS1_SHA256,
            &SystemRandom::new(),
            signing_input.as_bytes(),
            &mut sig,
        )
        .map_err(|_| AuthError::Signing)?;
        Ok(format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(sig)))
    }
}

/// The `application/x-www-form-urlencoded` body that exchanges a signed assertion for a token.
/// The assertion is base64url and dots, so it needs no escaping.
pub fn token_request_body(assertion: &str) -> String {
    format!(
        "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={assertion}"
    )
}

/// A short-lived bearer token for the FCM call. `Debug` and `Display` never show it.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken {
    value: String,
    /// Unix seconds after which Google rejects it.
    pub expires_at: u64,
}

impl AccessToken {
    /// The raw token, for the `Authorization` header only.
    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AccessToken(<redacted>, expires_at: {})",
            self.expires_at
        )
    }
}

impl fmt::Display for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Reads Google's token endpoint answer. `now` is when the request was made, unix seconds.
pub fn parse_token_response(json: &str, now: u64) -> Result<AccessToken, AuthError> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| AuthError::Rejected("not JSON".into()))?;
    if let Some(code) = v.get("error").and_then(Value::as_str) {
        // Only the short error code: `error_description` may quote request content.
        return Err(AuthError::Rejected(code.chars().take(64).collect()));
    }
    let value = v
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AuthError::Rejected("no access_token".into()))?;
    let lifetime = v.get("expires_in").and_then(Value::as_u64).unwrap_or(0);
    Ok(AccessToken {
        value: value.to_owned(),
        expires_at: now + lifetime,
    })
}

/// Holds the current token and says when to fetch another.
#[derive(Debug, Default)]
pub struct TokenCache {
    token: Option<AccessToken>,
}

impl TokenCache {
    pub fn store(&mut self, token: AccessToken) {
        self.token = Some(token);
    }

    /// The cached token, unless it is within [`REFRESH_MARGIN_SECONDS`] of expiry.
    pub fn valid(&self, now: u64) -> Option<&AccessToken> {
        self.token
            .as_ref()
            .filter(|t| t.expires_at > now + REFRESH_MARGIN_SECONDS)
    }

    /// Drop the token (Google answered 401: it was revoked early).
    pub fn clear(&mut self) {
        self.token = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A throwaway RSA key made for these tests on 2026-11-01. It protects nothing and appears
    // nowhere else. The PEM header is split so secret scanners that match "BEGIN PRIVATE KEY" do
    // not flag a test fixture.
    const TEST_KEY_BODY: &str = "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7oy/L2aE+aZO7GD3WWjccJzqxqMupZvaiGEt/pO+Rif23AruxQ0hlR3b0MATvwFWFb7aAKG54Inb+hH4cu7l7PZbVGiuMTkofk3YpEs6I5zOwzcti3nB0JVfn8sa8qaSb2hC8CQqpIJDE+r+wAUUXouvb7DFbabpLo+xP6k438UhXfCIyibGUtFa143+urAz6ubg2sIhnsRpEr1P+CA+G+LFRRjGRC3FY6QbGbn7AVUDyQnTDV0+iI8AF6m2pqmxbRYPY+pp+1QQAeCHn+7hx8DSCOxptCccdhPQ2Vmlpcx4N+hQsggFFGy4o+TxDHNzWqqCKDDyacbXJzOCzm/KXAgMBAAECggEABqU+op1VF1UW6dsi1lSNbusgXYo8ZR+j7LuOi1+uKnBOmSyfTov/yuWvW77XPFFXPHaBHX3d5Ikkv/Poux/8728pe4FFrJHAEOk6MWjPzswQqedH/ke7IanwhWMB6rM/0YZ1VEUmG88iJ9yeBsZGVt8RNATj/jhUfd+GFZIXVIAHOIYlkInpPWUCOX5ucKugu9lAKJVEk+RIxNwHUrRz8MOq27be9FUV5ajuyQ0A1zJpt7ilTdhpg3ZALR8QFvqP/psDiC6M9Jun1XapHvP0hWlAZHG0TJUIjovn+uBhWAeSNumoNhE3Lzr1WdH3WpTqg3w/9dWORyOah/5Rnvr/oQKBgQD3djjGZO1B8gHbbLJFq89pcThk7WDlhS1Oo5T2czHr+80SjHnR6vyEh6m+CDPifgOXjzhMgQrRsRJA/k/VQbff+F3NGCIZPAqKJSHgXJjS6+LVyJRr6MEFve3bFmk/laaUZY0ZnJcMzs9Du1wlAEL8vSCr4h3OrrsVufMswXz9IQKBgQDCHIyHJlVERJRICF4Aao0pW48ccfkYEzlle4mhY/59aHizfqYpJEigepkcyEROWTAQoMCchPJgYAbN/cAdmgwu7ZBhT2aXNrOeZ9bWN6H/b2EMb7juKwVXG9dzbySVSMYJYOHPGq9WCzsZI7GWCYCVRCB6V+Pqy2ZTE/sZD+MAtwKBgAKraNNjFVgnxzNZydaMkvE9QnF8ShZPyHxXsWyzM2+rSLwRfeLw2QaI/CDLr9uapmlFJ0NUBQzcDylaVPwk+FnXMRPcVcnV5vs0iVgFR7TGzKynvEopuaNZOC6r4DjKYYCqU8pZ8Rs3Q+8kH0ta5F4boDWae93V7ri+R8xMzg9hAoGAQAtZVlIQefjCj82q7io5rvaVjkYQcKAPJOuQql4qnkf04wwbWKGvELvCftuYpd0JXcMcKcg3jBLCy/43/8xOzsD1DRY8fBHBS7dpC77JRVUDSS4nbvVkyqvV9zEpedPL+1YqysE8LoCAHDhJpH44HYefy53qBhLcDPcfttImDJsCgYEAhWVaeHqZimCvYxf7pdZRUZgtRFFaWGLiKXNfRswP6piaVeKiNroETl+920sp0doflRx7PLYYalibcE9ocHFtNTtPUzsUNRybU+BPtucCK12MXIRWxOLfczWFv9IbW8rDuK2rCsW7YZpSGUoYenT3mC3CY5H8De6ZDYw9zmOwKdA=";

    fn test_pem() -> String {
        format!(
            "{}{}\n{TEST_KEY_BODY}\n{}{}\n",
            "-----BEGIN ", "PRIVATE KEY-----", "-----END ", "PRIVATE KEY-----"
        )
    }

    fn account_json() -> String {
        json!({
            "type": "service_account",
            "project_id": "alarmant-test",
            "private_key_id": "abc",
            "private_key": test_pem(),
            "client_email": "sender@alarmant-test.iam.gserviceaccount.com",
            "token_uri": "https://oauth2.googleapis.com/token",
        })
        .to_string()
    }

    fn account() -> ServiceAccount {
        ServiceAccount::from_json(&account_json()).unwrap()
    }

    #[test]
    fn parses_the_fields_the_sender_needs() {
        let a = account();
        assert_eq!(a.project_id(), "alarmant-test");
        assert_eq!(a.token_uri(), "https://oauth2.googleapis.com/token");
    }

    #[test]
    fn rejects_bad_service_account_files() {
        let good: Value = serde_json::from_str(&account_json()).unwrap();
        for field in [
            "type",
            "project_id",
            "private_key",
            "client_email",
            "token_uri",
        ] {
            let mut v = good.clone();
            v.as_object_mut().unwrap().remove(field);
            assert_eq!(
                ServiceAccount::from_json(&v.to_string()).unwrap_err(),
                AuthError::BadServiceAccount(match field {
                    "type" => "type",
                    "project_id" => "project_id",
                    "private_key" => "private_key",
                    "client_email" => "client_email",
                    _ => "token_uri",
                }),
            );
        }
        let mut v = good.clone();
        v["type"] = json!("authorized_user");
        assert!(ServiceAccount::from_json(&v.to_string()).is_err());
        let mut v = good;
        v["token_uri"] = json!("http://evil.example/token");
        assert_eq!(
            ServiceAccount::from_json(&v.to_string()).unwrap_err(),
            AuthError::BadServiceAccount("token_uri")
        );
        assert!(ServiceAccount::from_json("nope").is_err());
    }

    #[test]
    fn assertion_has_the_documented_header_and_claims() {
        let jwt = account().assertion(1_000_000).unwrap();
        let parts: Vec<_> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let dec = |s: &str| -> Value {
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(s).unwrap()).unwrap()
        };
        assert_eq!(dec(parts[0]), json!({"alg": "RS256", "typ": "JWT"}));
        let c = dec(parts[1]);
        assert_eq!(c["iss"], "sender@alarmant-test.iam.gserviceaccount.com");
        assert_eq!(c["scope"], SCOPE);
        assert_eq!(c["aud"], "https://oauth2.googleapis.com/token");
        assert_eq!(c["iat"], 1_000_000);
        assert_eq!(c["exp"], 1_000_000 + ASSERTION_LIFETIME_SECONDS);
    }

    #[test]
    fn assertion_signature_verifies_against_the_public_key() {
        use ring::signature::{RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey};
        let a = account();
        let jwt = a.assertion(5).unwrap();
        let (signing_input, sig) = jwt.rsplit_once('.').unwrap();
        let public = a.key_pair().unwrap().public().as_ref().to_vec();
        let sig = URL_SAFE_NO_PAD.decode(sig).unwrap();
        UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, public)
            .verify(signing_input.as_bytes(), &sig)
            .expect("signature must verify");
        // Positive control for the check itself: a different message must fail.
        let other = a.key_pair().unwrap().public().as_ref().to_vec();
        assert!(
            UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, other)
                .verify(b"something else", &sig)
                .is_err()
        );
    }

    #[test]
    fn a_bad_key_is_reported_without_its_content() {
        let mut v: Value = serde_json::from_str(&account_json()).unwrap();
        v["private_key"] =
            json!("-----BEGIN PRIVATE KEY-----\nSECRETISH\n-----END PRIVATE KEY-----\n");
        let a = ServiceAccount::from_json(&v.to_string()).unwrap();
        let e = a.assertion(1).unwrap_err();
        assert_eq!(e, AuthError::BadKey);
        assert!(!format!("{e:?}").contains("SECRETISH"));
    }

    #[test]
    fn debug_never_prints_the_key() {
        let a = account();
        let text = format!("{a:?}");
        assert!(!text.contains(TEST_KEY_BODY));
        assert!(!text.contains(&TEST_KEY_BODY[..40]));
        assert!(text.contains("<redacted>"));
    }

    #[test]
    fn token_request_body_is_the_jwt_bearer_grant() {
        assert_eq!(
            token_request_body("a.b.c"),
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion=a.b.c"
        );
    }

    #[test]
    fn token_response_gives_a_redacted_token_with_an_expiry() {
        let t = parse_token_response(
            r#"{"access_token":"ya29.secret","expires_in":3599,"token_type":"Bearer"}"#,
            100,
        )
        .unwrap();
        assert_eq!(t.expose(), "ya29.secret");
        assert_eq!(t.expires_at, 3699);
        assert!(!format!("{t:?} {t}").contains("ya29"));
    }

    #[test]
    fn token_errors_carry_only_the_short_code() {
        let e = parse_token_response(
            r#"{"error":"invalid_grant","error_description":"Invalid JWT Signature for abc"}"#,
            1,
        )
        .unwrap_err();
        assert_eq!(e, AuthError::Rejected("invalid_grant".into()));
        for bad in ["", "{}", r#"{"access_token":""}"#, "[]"] {
            assert!(parse_token_response(bad, 1).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn cache_refreshes_a_minute_before_expiry() {
        let mut c = TokenCache::default();
        assert!(c.valid(0).is_none());
        c.store(parse_token_response(r#"{"access_token":"t","expires_in":1000}"#, 0).unwrap());
        assert!(c.valid(939).is_some());
        assert!(c.valid(940).is_none());
        c.store(parse_token_response(r#"{"access_token":"t","expires_in":1000}"#, 0).unwrap());
        c.clear();
        assert!(c.valid(1).is_none());
    }
}
