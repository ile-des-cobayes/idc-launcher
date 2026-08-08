use portablemc::forge::{self, Loader, Version as ForgeVersion};
use portablemc::base::Game;
use reqwest::Client;
use serde::Deserialize;
use serde_json;
use std::path::PathBuf;
use std::time::Duration;

/// URL de base de notre API d'authentification Yggdrasil
/// Injectée depuis le .env via build.rs
/// IMPORTANT : Doit finir par un / pour que authlib-injector accède à /metadata
const AUTH_API_BASE: &str = env!("YGGDRASIL_API_BASE");

/// Retourne l'URL de l'API avec un slash final garanti
fn auth_api_base_with_slash() -> String {
    if AUTH_API_BASE.ends_with('/') {
        AUTH_API_BASE.to_string()
    } else {
        format!("{}/", AUTH_API_BASE)
    }
}

/// URL pour télécharger authlib-injector.jar
/// Injectée depuis le .env via build.rs
const AUTHLIB_INJECTOR_URL: &str = env!("AUTHLIB_INJECTOR_URL");

/// Ce que le launcher a obtenu après authentification Discord + appel
/// à notre serveur Yggdrasil (`/authserver/authenticate`).
#[derive(Debug, Clone)]
pub struct SessionYggdrasil {
    pub username: String,
    pub uuid_no_dashes: String,
    pub access_token: String,
}

/// Réponse de l'API /authserver/authenticate
#[derive(Debug, Deserialize)]
struct YggdrasilAuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "selectedProfile")]
    selected_profile: SelectedProfile,
    #[serde(rename = "clientToken")]
    client_token: Option<String>,
    #[serde(rename = "availableProfiles")]
    available_profiles: Option<Vec<AvailableProfile>>,
}

#[derive(Debug, Deserialize)]
struct SelectedProfile {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct AvailableProfile {
    id: String,
    name: String,
}

/// Dossier où va vivre notre launcher, positionné au même niveau que le
/// dossier `.minecraft` du launcher officiel, selon l'OS :
/// - Windows : `%APPDATA%\idc-launcher`      (à côté de `%APPDATA%\.minecraft`)
/// - macOS   : `~/Library/Application Support/idc-launcher` (à côté de `.../minecraft`)
/// - Linux   : `~/idc-launcher`               (à côté de `~/.minecraft`)
fn dossier_jeu() -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    let base = dirs::home_dir();

    #[cfg(not(target_os = "linux"))]
    let base = dirs::config_dir();

    let base = base.ok_or_else(|| "Impossible de déterminer le dossier utilisateur".to_string())?;
    Ok(base.join("idc-launcher"))
}

/// Télécharge authlib-injector.jar s'il n'existe pas déjà dans le dossier du jeu
/// Cette fonction est asynchrone car elle utilise reqwest en mode async
async fn telecharger_authlib_injector_async(dossier: &PathBuf) -> Result<PathBuf, String> {
    let authlib_jar = dossier.join("authlib-injector.jar");
    
    if authlib_jar.exists() {
        return Ok(authlib_jar);
    }
    
    println!("Téléchargement de authlib-injector.jar...");
    
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Impossible de créer le client HTTP : {}", e))?;
    
    let response = client
        .get(AUTHLIB_INJECTOR_URL)
        .send()
        .await
        .map_err(|e| format!("Impossible de télécharger authlib-injector.jar : {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Erreur lors du téléchargement de authlib-injector.jar : HTTP {}", response.status()));
    }
    
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Impossible de lire la réponse : {}", e))?;
    
    tokio::fs::write(&authlib_jar, &bytes)
        .await
        .map_err(|e| format!("Impossible d'écrire authlib-injector.jar : {}", e))?;
    
    println!("authlib-injector.jar téléchargé avec succès.");
    Ok(authlib_jar)
}

/// Appelle notre API Yggdrasil pour échanger le token Discord contre une session Minecraft
async fn authentifier_avec_api_yggdrasil(discord_token: &str) -> Result<SessionYggdrasil, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Impossible de créer le client HTTP : {}", e))?;
    
    let response = client
        .post(format!("{}/authserver/authenticate", auth_api_base_with_slash()))
        .json(&serde_json::json!({
            "accessToken": discord_token
        }))
        .send()
        .await
        .map_err(|e| format!("Impossible de contacter l'API d'authentification : {}", e))?;
    
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Erreur d'authentification Yggdrasil : HTTP {} - {}", status, error_text));
    }
    
    // Debug: Afficher la réponse brute pour le débogage
    let response_text = response.text().await.map_err(|e| format!("Impossible de lire la réponse : {}", e))?;
    
    let auth_response: YggdrasilAuthResponse = serde_json::from_str(&response_text)
        .map_err(|e| format!("Impossible de parser la réponse d'authentification : {} | Réponse: {}", e, response_text))?;
    
    Ok(SessionYggdrasil {
        username: auth_response.selected_profile.name,
        uuid_no_dashes: auth_response.selected_profile.id,
        access_token: auth_response.access_token,
    })
}

/// Remplace la valeur qui suit un flag donné dans une liste d'arguments
/// (ex: trouve "--uuid" et remplace l'élément juste après).
fn patcher_identite(game: &mut Game, session: &SessionYggdrasil) {
    // Remplace les arguments d'authentification dans game_args
    replace_arg_value(&mut game.game_args, "--username", &session.username);
    replace_arg_value(&mut game.game_args, "--uuid", &session.uuid_no_dashes);
    replace_arg_value(&mut game.game_args, "--accessToken", &session.access_token);
}

/// Remplace la valeur qui suit un flag donné dans une liste d'arguments
/// (ex: trouve "--uuid" et remplace l'élément juste après).
fn replace_arg_value(args: &mut Vec<String>, flag: &str, value: &str) {
    if let Some(pos) = args.iter().position(|a| a == flag) {
        if pos + 1 < args.len() {
            args[pos + 1] = value.to_string();
        }
    }
}

/// Prépare puis démarre Minecraft avec authentification Yggdrasil.
/// Les étapes sont remontées au launcher par le callback.
pub fn lancer_jeu_bloquant_avec_progress<F>(
    discord_token: Option<&str>,
    pseudo: &str,
    on_progress: F,
) -> Result<(), String>
where
    F: Fn(&str, u8, &str, &str),
{
    on_progress(
        "minecraft",
        70,
        "Préparation de Minecraft",
        "Vérification du client 1.21.1",
    );
    let dossier = dossier_jeu()?;

    // portablemc canonicalise ce chemin en interne (voir l'erreur
    // "No such file or directory @ canonicalize") : il DOIT déjà exister
    // sur le disque avant qu'on le passe à `set_mc_dir`.
    std::fs::create_dir_all(&dossier).map_err(|e| {
        format!("Impossible de créer le dossier du jeu ({}) : {e}", dossier.display())
    })?;

    // On récupère le handle tokio pour les appels async bloquants
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            // Si on n'est pas dans un contexte tokio, créer un runtime temporaire
            let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            rt.handle().clone()
        }
    };

    // Télécharger authlib-injector.jar s'il n'existe pas
    on_progress(
        "authlib",
        75,
        "Préparation de l'authentification",
        "Téléchargement de authlib-injector",
    );
    let _authlib_jar = handle.block_on(async {
        telecharger_authlib_injector_async(&dossier).await
    })?;

    on_progress(
        "yggdrasil",
        80,
        "Authentification Yggdrasil",
        "Connexion via notre serveur d'authentification",
    );
    
    // Appel bloquant à l'API Yggdrasil pour récupérer la session
    let session = handle.block_on(async {
        let token = discord_token
            .ok_or("Token Discord non disponible")?;
        authentifier_avec_api_yggdrasil(token).await
    })?;

    on_progress(
        "neoforge",
        85,
        "Installation de NeoForge",
        "Préparation des bibliothèques et du profil de jeu",
    );
    let mut installer = forge::Installer::new(Loader::NeoForge, ForgeVersion::Name("21.1.232".to_string()));

    {
        let mojang = installer.mojang_mut();
        mojang.set_version("1.21.1");
        // Identité temporaire, juste pour que portablemc génère une commande valide.
        // Elle sera écrasée juste après par la vraie identité Yggdrasil.
        mojang.set_auth_offline_username(pseudo);
        mojang.base_mut().set_mc_dir(&dossier);

    }

    let mut game = installer
        .install(())
        .map_err(|e| format!("Erreur lors de l'installation : {e}"))?;

    // On remplace username/uuid/accessToken par les vraies valeurs Yggdrasil.
    patcher_identite(&mut game, &session);

    // On ajoute le javaagent authlib-injector, indispensable pour que le client
    // parle à notre serveur d'auth au lieu de Mojang.
    let authlib_jar = dossier.join("authlib-injector.jar");
    
    game.jvm_args.insert(
        0,
        format!("-javaagent:{}={}", authlib_jar.display(), auth_api_base_with_slash()),
    );

    on_progress(
        "starting",
        96,
        "Démarrage du jeu",
        "Lancement de Minecraft avec ton profil",
    );
    // Le launcher ne doit pas rester verrouillé pendant toute la partie :
    // `spawn` confirme le démarrage de Java puis rend immédiatement la main.
    game.spawn()
        .map_err(|e| format!("Erreur lors du lancement du jeu : {e}"))?;

    on_progress(
        "started",
        100,
        "Minecraft est lancé",
        "Bon jeu sur L'île des Cobayes !",
    );
    Ok(())
}
