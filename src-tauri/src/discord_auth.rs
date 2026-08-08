use keyring::Entry;
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Clé utilisée pour stocker le refresh token dans le keychain OS
const KEYRING_SERVICE: &str = "idc-launcher";
const KEYRING_USER: &str = "discord_refresh_token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
}

/// Réponse complète de Discord OAuth2 (inclut access_token et refresh_token)
#[derive(Debug, Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: String,
}

pub struct DiscordAuth {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    pkce_verifier: Arc<Mutex<Option<PkceCodeVerifier>>>,
    csrf_token: Arc<Mutex<Option<CsrfToken>>>,
}

impl DiscordAuth {
    pub fn new() -> Self {
        // Valeurs injectées au moment de la compilation par build.rs
        // (lecture du .env -> cargo:rustc-env). Choix assumé : ces
        // valeurs sont embarquées en clair dans le binaire final.
        Self {
            client_id: env!("DISCORD_CLIENT_ID").to_string(),
            client_secret: env!("DISCORD_CLIENT_SECRET").to_string(),
            redirect_uri: env!("DISCORD_REDIRECT_URI").to_string(),
            pkce_verifier: Arc::new(Mutex::new(None)),
            csrf_token: Arc::new(Mutex::new(None)),
        }
    }

    fn build_client(&self) -> oauth2::basic::BasicClient {
        oauth2::basic::BasicClient::new(
            oauth2::ClientId::new(self.client_id.clone()),
            Some(oauth2::ClientSecret::new(self.client_secret.clone())),
            oauth2::AuthUrl::new("https://discord.com/oauth2/authorize".to_string()).unwrap(),
            Some(oauth2::TokenUrl::new("https://discord.com/api/oauth2/token".to_string()).unwrap()),
        )
            .set_redirect_uri(RedirectUrl::new(self.redirect_uri.clone()).unwrap())
    }

    pub async fn get_auth_url(&self) -> (String, String) {
        let client = self.build_client();

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let csrf_token = CsrfToken::new_random();

        *self.pkce_verifier.lock().await = Some(pkce_verifier);
        *self.csrf_token.lock().await = Some(csrf_token.clone());

        let (auth_url, _csrf_token) = client
            .authorize_url(|| csrf_token.clone())
            .add_scope(oauth2::Scope::new("identify".to_string()))
            .add_scope(oauth2::Scope::new("email".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        let csrf_secret = csrf_token.secret().as_str().to_string();
        (auth_url.to_string(), csrf_secret)
    }

    /// Stocke le refresh token dans le keychain OS de manière sécurisée
    fn store_refresh_token(&self, refresh_token: &str) -> Result<(), String> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| format!("Impossible d'accéder au keychain : {}", e))?;
        entry
            .set_password(refresh_token)
            .map_err(|e| format!("Impossible de stocker le refresh token : {}", e))?;
        Ok(())
    }

    /// Récupère le refresh token depuis le keychain OS
    fn get_refresh_token(&self) -> Result<Option<String>, String> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| format!("Impossible d'accéder au keychain : {}", e))?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Impossible de récupérer le refresh token : {}", e)),
        }
    }

    /// Supprime le refresh token du keychain
    fn clear_refresh_token(&self) -> Result<(), String> {
        let entry = Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| format!("Impossible d'accéder au keychain : {}", e))?;
        entry
            .set_password("")
            .map_err(|e| format!("Impossible de supprimer le refresh token : {}", e))?;
        Ok(())
    }

    /// Échange un code d'autorisation Discord contre des tokens
    /// Retourne (access_token, refresh_token)
    pub async fn exchange_code(&self, code: &str) -> Result<(String, String), String> {
        let client = self.build_client();

        let pkce_verifier = self.pkce_verifier.lock().await;
        let verifier = pkce_verifier.as_ref().ok_or_else(|| "PKCE verifier not set".to_string())?;
        let verifier_secret = verifier.secret().clone();

        let token = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(PkceCodeVerifier::new(verifier_secret))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let access_token = token.access_token().secret().clone();
        let refresh_token = token
            .refresh_token()
            .map(|rt| rt.secret().clone())
            .ok_or_else(|| "No refresh token received from Discord".to_string())?;

        // Stocker le refresh token pour les prochains lancements
        self.store_refresh_token(&refresh_token)?;

        Ok((access_token, refresh_token))
    }

    /// Rafraîchit l'access token Discord en utilisant le refresh token stocké
    pub async fn refresh_access_token(&self) -> Result<String, String> {
        let refresh_token = self
            .get_refresh_token()?
            .ok_or_else(|| "Aucun refresh token disponible. Veuillez vous reconnecter.".to_string())?;

        let client = reqwest::Client::new();
        let params = [
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.clone()),
        ];

        let response = client
            .post("https://discord.com/api/oauth2/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Impossible de contacter Discord : {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            
            // Si le refresh token est invalide, on le supprime
            if status == reqwest::StatusCode::UNAUTHORIZED || error_text.contains("invalid_grant") {
                self.clear_refresh_token()?;
            }
            
            return Err(format!(
                "Erreur lors du rafraîchissement du token Discord : HTTP {} - {}",
                status, error_text
            ));
        }

        let token_response: DiscordTokenResponse = response
            .json()
            .await
            .map_err(|e| format!("Impossible de parser la réponse Discord : {}", e))?;

        // Mettre à jour le refresh token si un nouveau a été fourni
        if !token_response.refresh_token.is_empty() {
            self.store_refresh_token(&token_response.refresh_token)?;
        }

        Ok(token_response.access_token)
    }

    /// Obtient un access token Discord valide (frais ou rafraîchi)
    pub async fn get_valid_access_token(&self) -> Result<String, String> {
        // Essayer de rafraîchir avec le refresh token existant
        match self.refresh_access_token().await {
            Ok(token) => Ok(token),
            Err(_) => {
                // Pas de refresh token valide, il faudra faire une nouvelle auth
                Err("Token Discord expiré. Veuillez vous reconnecter.".to_string())
            }
        }
    }

    pub async fn get_user_info(&self, access_token: &str) -> Result<DiscordUser, String> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://discord.com/api/users/@me")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Failed to get user info: {}", response.status()));
        }

        let user_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        Ok(DiscordUser {
            id: user_data["id"].as_str().unwrap_or("").to_string(),
            username: user_data["username"].as_str().unwrap_or("").to_string(),
            discriminator: user_data["discriminator"].as_str().unwrap_or("").to_string(),
            avatar: user_data["avatar"].as_str().map(|s| s.to_string()),
            access_token: None,
        })
    }
}