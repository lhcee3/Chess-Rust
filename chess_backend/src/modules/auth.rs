use std::sync::RwLock;

pub struct FirebaseConfig {
    pub api_key: RwLock<String>,
    pub project_id: RwLock<String>,
}

impl FirebaseConfig {
    pub fn new() -> Self {
        FirebaseConfig {
            api_key: RwLock::new(String::new()),
            project_id: RwLock::new(String::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserCtx {
    pub uid: String,
    pub email: String,
}

pub async fn verify_token(id_token: &str, config: &FirebaseConfig) -> Result<UserCtx, String> {
    let api_key = config.api_key.read().map_err(|e| format!("lock error: {}", e))?.clone();
    if api_key.is_empty() {
        let env_key = std::env::var("FIREBASE_API_KEY").unwrap_or_default();
        if !env_key.is_empty() {
            return verify_with_lookup(id_token, &env_key).await;
        }
        return Err("Firebase API key not configured".to_string());
    }
    verify_with_lookup(id_token, &api_key).await
}

async fn verify_with_lookup(id_token: &str, api_key: &str) -> Result<UserCtx, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://identitytoolkit.googleapis.com/v1/accounts:lookup?key={}",
        api_key
    );

    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "idToken": id_token }))
        .send()
        .await
        .map_err(|e| format!("Firebase lookup request failed: {}", e))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Firebase response: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "Firebase auth failed ({}): {}",
            status,
            body["error"]["message"].as_str().unwrap_or("unknown")
        ));
    }

    let users = body["users"]
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or_else(|| "No user data in Firebase response".to_string())?;

    let uid = users["localId"]
        .as_str()
        .ok_or_else(|| "Missing uid in Firebase response".to_string())?
        .to_string();
    let email = users["email"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let verified = users["emailVerified"].as_bool().unwrap_or(false);

    if !verified && !email.is_empty() {
        return Err("Email not verified".to_string());
    }

    Ok(UserCtx { uid, email })
}
