use crate::discord_auth::{DiscordAuth, DiscordUser};
use std::collections::HashMap;
use std::sync::Arc;
use tiny_http::{Header, Response, Server};
use tokio::sync::Mutex;

pub struct CallbackServer {
    // On stocke directement le résultat final (utilisateur ou erreur) :
    // l'échange code -> token -> profil se fait maintenant ici, une seule fois.
    user_result: Arc<Mutex<Option<Result<DiscordUser, String>>>>,
    server: Arc<Mutex<Option<Arc<Server>>>>,
}

impl CallbackServer {
    pub fn new() -> Self {
        Self {
            user_result: Arc::new(Mutex::new(None)),
            server: Arc::new(Mutex::new(None)),
        }
    }

    /// `discord_auth` est partagé pour pouvoir échanger le code reçu sur le
    /// callback avant même de répondre au navigateur (on peut ainsi afficher
    /// le pseudo et l'avatar directement sur la page de confirmation).
    pub async fn start(&self, discord_auth: Arc<DiscordAuth>) -> Result<String, String> {
        // Ferme proprement un éventuel serveur précédent encore en attente
        // (sinon le port reste occupé et le prochain bind échoue).
        self.stop().await;

        let user_result = self.user_result.clone();

        let port = 27849;
        let bind_address = format!("127.0.0.1:{}", port);

        let server = Server::http(&bind_address)
            .map_err(|e| format!("Impossible de démarrer le serveur local sur {bind_address} : {e}"))?;
        let server = Arc::new(server);
        *self.server.lock().await = Some(server.clone());

        // On garde un handle vers le runtime tokio courant pour pouvoir
        // exécuter l'échange async (reqwest) depuis le thread bloquant de
        // tiny_http.
        let rt_handle = tokio::runtime::Handle::current();

        tokio::task::spawn_blocking(move || {
            for request in server.incoming_requests() {
                let url = request.url().to_string();
                if url.starts_with("/callback?") {
                    let query = url.strip_prefix("/callback?").unwrap_or("");
                    let params: HashMap<String, String> = query
                        .split('&')
                        .filter_map(|s| {
                            let mut parts = s.split('=');
                            Some((parts.next()?.to_string(), parts.next()?.to_string()))
                        })
                        .collect();

                    let outcome: Result<DiscordUser, String> = if let Some(code) = params.get("code") {
                        let discord_auth = discord_auth.clone();
                        let code = code.clone();
                        rt_handle.block_on(async move {
                            let (access_token, _refresh_token) = discord_auth.exchange_code(&code).await?;
                            let user_info = discord_auth.get_user_info(&access_token).await?;
                            Ok(DiscordUser {
                                access_token: Some(access_token),
                                ..user_info
                            })
                        })
                    } else {
                        let message = params
                            .get("error_description")
                            .or_else(|| params.get("error"))
                            .cloned()
                            .unwrap_or_else(|| "Autorisation refusée sur Discord.".to_string());
                        Err(message)
                    };

                    let html = build_response_html(&outcome);
                    *user_result.blocking_lock() = Some(outcome);

                    // C'est ici le vrai correctif du bug d'affichage : sans cet
                    // en-tête, tiny_http répond en text/plain et le navigateur
                    // affiche le code HTML brut au lieu de le rendre.
                    let header =
                        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
                    let response = Response::from_string(html)
                        .with_status_code(200)
                        .with_header(header);

                    let _ = request.respond(response);
                    break;
                }
            }
        });

        Ok(bind_address)
    }

    /// Attend le résultat de l'authentification (utilisateur Discord déjà
    /// résolu, ou message d'erreur). Ne fait plus d'appel réseau ici : tout a
    /// déjà été fait dans le handler du callback.
    pub async fn wait_for_auth(&self) -> Option<Result<DiscordUser, String>> {
        let timeout = tokio::time::Duration::from_secs(300);

        tokio::time::timeout(timeout, async {
            loop {
                {
                    let result = self.user_result.lock().await;
                    if result.is_some() {
                        return result.clone();
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        })
            .await
            .ok()
            .flatten()
    }

    /// Débloque et libère un éventuel serveur encore en attente d'une requête.
    /// tiny_http fournit `unblock()` justement pour ce cas : ça fait sortir
    /// `incoming_requests()` de sa boucle bloquante côté thread, qui se termine
    /// alors naturellement et libère le port.
    pub async fn stop(&self) {
        if let Some(server) = self.server.lock().await.take() {
            server.unblock();
        }
    }

    pub async fn reset(&self) {
        *self.user_result.lock().await = None;
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CSS_COMMUN: &str = r#"
:root {
  --bg-void: #07100b;
  --bg-surface-2: #16291b;
  --bg-surface-3: #1e3523;
  --accent: #79c95c;
  --accent-bright: #c4f18d;
  --accent-dim: #3f8548;
  --text-primary: #f2f8ef;
  --text-muted: #9eb6a1;
  --online: #7ee580;
  --danger: #ff5a6e;
  --border: rgba(210, 241, 213, 0.13);
  --font-display: "Space Grotesk", sans-serif;
  --font-body: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
html, body {
  margin: 0;
  height: 100%;
  background: var(--bg-void);
  color: var(--text-primary);
  font-family: var(--font-body);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.decor { position: fixed; inset: 0; overflow: hidden; pointer-events: none; z-index: 0; }
.shard {
  position: absolute;
  background: linear-gradient(135deg, var(--accent) 0%, var(--accent-dim) 100%);
  clip-path: polygon(50% 0%, 100% 38%, 82% 100%, 18% 100%, 0% 38%);
  opacity: 0.16;
  animation: flotter 9s ease-in-out infinite;
}
.shard-1 { top: 14%; left: 12%; width: 40px; height: 40px; }
.shard-2 { top: 70%; left: 80%; width: 64px; height: 64px; animation-delay: 1.5s; }
.shard-3 { top: 22%; left: 84%; width: 30px; height: 30px; animation-delay: 3s; }
@keyframes flotter {
  0%, 100% { transform: translateY(0) rotate(0deg); }
  50% { transform: translateY(-18px) rotate(8deg); }
}
.carte {
  position: relative;
  z-index: 1;
  width: 100%;
  max-width: 380px;
  margin: 24px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  text-align: center;
  padding: 40px 32px;
  background: linear-gradient(160deg, var(--bg-surface-2), var(--bg-surface-3));
  border: 1px solid var(--border);
  border-radius: 20px;
  box-shadow: 0 24px 60px -20px rgba(70, 158, 65, 0.35);
}
.marque-icone {
  width: 56px;
  height: 56px;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 15px;
  background: linear-gradient(150deg, var(--bg-surface-2), var(--bg-surface-3));
  border: 1px solid var(--border);
}
.coche {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--bg-void);
  background: var(--online);
}
.coche--erreur { background: var(--danger); }
h1 {
  margin: 0;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 22px;
}
.profil {
  display: flex;
  align-items: center;
  gap: 10px;
  background: rgba(10, 9, 18, 0.4);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 6px 18px 6px 6px;
}
.avatar {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  object-fit: cover;
}
.avatar-lettre {
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 13px;
  background: linear-gradient(150deg, var(--accent), var(--accent-dim));
}
.profil-nom { font-size: 13.5px; color: var(--text-primary); }
.profil-nom strong { color: var(--accent-bright); }
.sous-texte { margin: 0; font-size: 13.5px; line-height: 1.5; color: var(--text-muted); }
.sous-texte--muted { font-size: 12.5px; }
"#;

fn build_response_html(outcome: &Result<DiscordUser, String>) -> String {
    match outcome {
        Ok(user) => {
            let avatar_html = if let Some(avatar) = &user.avatar {
                let ext = if avatar.starts_with("a_") { "gif" } else { "png" };
                format!(
                    r#"<img class="avatar" src="https://cdn.discordapp.com/avatars/{}/{}.{ext}?size=128" alt="Avatar" />"#,
                    user.id, avatar
                )
            } else {
                let initiale = user
                    .username
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                format!(r#"<div class="avatar avatar-lettre">{}</div>"#, initiale)
            };

            format!(
                r#"<!doctype html>
<html lang="fr">
<head>
<meta charset="UTF-8" />
<title>Connexion réussie — L'île des Cobayes</title>
<link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@600;700&family=Inter:wght@400;500;600&display=swap" rel="stylesheet" />
<style>{css}</style>
</head>
<body>
<div class="decor">
  <span class="shard shard-1"></span>
  <span class="shard shard-2"></span>
  <span class="shard shard-3"></span>
</div>
<div class="carte">
  <div class="marque-icone">LC</div>
  <div class="coche">
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
  </div>
  <h1>Connexion réussie</h1>
  <div class="profil">
    {avatar_html}
    <span class="profil-nom">Connecté en tant que <strong>{username}</strong></span>
  </div>
  <p class="sous-texte">Tu peux fermer cette fenêtre et retourner au launcher.</p>
</div>
</body>
</html>"#,
                css = CSS_COMMUN,
                avatar_html = avatar_html,
                username = html_escape(&user.username)
            )
        }
        Err(message) => format!(
            r#"<!doctype html>
<html lang="fr">
<head>
<meta charset="UTF-8" />
<title>Échec de connexion — L'île des Cobayes</title>
<link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@600;700&family=Inter:wght@400;500;600&display=swap" rel="stylesheet" />
<style>{css}</style>
</head>
<body>
<div class="decor">
  <span class="shard shard-1"></span>
  <span class="shard shard-2"></span>
  <span class="shard shard-3"></span>
</div>
<div class="carte">
  <div class="marque-icone">LC</div>
  <div class="coche coche--erreur">
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
  </div>
  <h1>Échec de la connexion</h1>
  <p class="sous-texte">{message}</p>
  <p class="sous-texte sous-texte--muted">Tu peux fermer cette fenêtre et réessayer depuis le launcher.</p>
</div>
</body>
</html>"#,
            css = CSS_COMMUN,
            message = html_escape(message)
        ),
    }
}
