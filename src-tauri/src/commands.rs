use crate::database::User;
use crate::discord_auth::DiscordUser;
use crate::news::NewsItem;
use crate::resources::{ResourceManager, SyncResult, FileInfo};
use crate::AppState;
use tauri::State;

/// Installe et lance le jeu pour le pseudo donné. Le travail bloquant
/// (installation + attente du process Java) tourne dans un thread dédié
/// via `spawn_blocking`, pour ne jamais geler le runtime tokio partagé
/// avec l'auth Discord et la DB.
///
/// La synchronisation des ressources (mods, configs, etc.) se fait AVANT
/// le lancement du jeu.
#[tauri::command]
pub async fn launch_game(username: String, state: State<'_, AppState>) -> Result<(), String> {
    let resource_manager = state.resource_manager.clone();
    crate::game::lancer_jeu_avec_ressources(username, resource_manager).await
}

/// Démarre le serveur de callback local et renvoie l'URL d'auth Discord
/// à ouvrir dans le navigateur système (le frontend s'en charge via
/// le plugin `opener`).
///
/// On passe `discord_auth` (cloné, c'est juste un Arc) au serveur de
/// callback : c'est lui qui échange désormais le code contre un profil
/// dès qu'il reçoit la requête, pour pouvoir afficher le pseudo/avatar
/// sur la page de confirmation.
#[tauri::command]
pub async fn start_discord_auth(state: State<'_, AppState>) -> Result<String, String> {
    state.callback_server.reset().await;
    state
        .callback_server
        .start(state.discord_auth.clone())
        .await?;
    let (url, _csrf) = state.discord_auth.get_auth_url().await;
    Ok(url)
}

/// Attend que l'utilisateur termine le flow dans le navigateur (jusqu'à
/// 5 minutes). L'échange code -> token -> profil a déjà été fait par le
/// serveur de callback lui-même (voir `callback_server.rs`) : un code
/// d'autorisation Discord est à usage unique, donc on ne le refait pas
/// ici, on récupère simplement le résultat déjà calculé.
#[tauri::command]
pub async fn complete_discord_auth(state: State<'_, AppState>) -> Result<DiscordUser, String> {
    let result = state
        .callback_server
        .wait_for_auth()
        .await
        .ok_or_else(|| "Authentification expirée ou annulée".to_string())?;

    state.callback_server.reset().await;

    result
}

#[tauri::command]
pub async fn get_user_by_discord_id(
    discord_id: String,
    state: State<'_, AppState>,
) -> Result<Option<User>, String> {
    let db_guard = state.db.lock().await;
    match db_guard.as_ref() {
        Some(db) => db.get_user_by_discord_id(&discord_id).await.map_err(|e| e.to_string()),
        None => Err("Base de données non connectée".to_string()),
    }
}

#[tauri::command]
pub async fn create_user(
    discord_id: String,
    username: String,
    state: State<'_, AppState>,
) -> Result<User, String> {
    let db_guard = state.db.lock().await;
    match db_guard.as_ref() {
        Some(db) => db.create_user(&discord_id, &username).await.map_err(|e| e.to_string()),
        None => Err("Base de données non connectée".to_string()),
    }
}

/// Synchronise toutes les ressources du jeu (mods, resource packs, configs)
/// - Installe le dossier base s'il n'existe pas
/// - Synchronise toujours le dossier updates
#[tauri::command]
pub async fn sync_resources(
    state: State<'_, AppState>,
) -> Result<SyncResult, String> {
    let manager = state.resource_manager.clone();
    manager.sync_game_resources().await
}

/// Vérifie si les ressources sont à jour
#[tauri::command]
pub async fn check_resources_up_to_date(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let manager = state.resource_manager.clone();
    manager.check_resources_up_to_date().await
}

/// Obtient la liste des fichiers disponibles sur le serveur pour un chemin donné
/// (utile pour le débogage ou l'affichage dans l'UI)
#[tauri::command]
pub async fn list_server_files(
    server_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileInfo>, String> {
    let manager = &state.resource_manager;
    let files = manager.fetch_remote_files(&server_path).await?;
    Ok(files.into_values().collect())
}

/// Obtient le chemin du dossier de jeu
#[tauri::command]
pub async fn get_game_directory() -> Result<String, String> {
    let path = ResourceManager::get_game_dir()?;
    Ok(path.to_string_lossy().to_string())
}

// ============================================================================
// Gestion des skins
// ============================================================================
//
// IMPORTANT : l'URL de l'API skin est désormais embarquée à la COMPILATION
// via env!(), exactement comme MYSQL_HOST/MYSQL_USER dans database.rs,
// plutôt que lue à l'exécution via std::env::var().
//
// Avant, `std::env::var("SKIN_API_URL").unwrap_or_else(|_| "http://localhost:3228")`
// retombait silencieusement sur localhost:3228 dès que l'app tournait en
// dehors d'un `cargo run` avec le .env chargé dans le shell (typiquement le
// binaire bundlé produit par `tauri build`) — l'app compilée n'a jamais accès
// à ton .env local, contrairement au terminal où tu lances `cargo run`.
//
// Il faut donc que ton build.rs (le même qui forwarde déjà MYSQL_USER,
// MYSQL_PASSWORD, MYSQL_HOST, MYSQL_DATABASE, RESOURCES_SERVER, etc. via
// `println!("cargo:rustc-env=...")`) forwarde aussi SKIN_API_URL depuis ton
// .env. Si `SKIN_API_URL` n'existe pas encore dans ton .env, ajoute par
// exemple : SKIN_API_URL=https://ouepamal.fr/skin-api
//
// Si build.rs ne fait pas ça encore pour cette variable précise, la
// compilation échouera avec une erreur explicite du type
// "environment variable `SKIN_API_URL` not defined" — ce qui est justement
// le comportement voulu : on préfère un échec net à la compilation plutôt
// qu'un fallback silencieux vers localhost en prod.
const SKIN_API_URL: &str = env!("SKIN_API_URL");

/// Upload un skin pour un utilisateur
#[tauri::command]
pub async fn upload_skin(
    discord_id: String,
    skin_bytes: Vec<u8>,
) -> Result<(), String> {
    use reqwest::multipart;
    use std::time::Duration;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let part = multipart::Part::bytes(skin_bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| e.to_string())?;

    let form = multipart::Form::new()
        .part("skin", part);

    let response = client
        .post(&format!("{}/api/upload-skin/{}", SKIN_API_URL, discord_id))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API Error: {}", error_text));
    }

    Ok(())
}

/// Supprime le skin custom d'un utilisateur
#[tauri::command]
pub async fn delete_skin(discord_id: String) -> Result<(), String> {
    use reqwest::Client;
    use std::time::Duration;

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .delete(&format!("{}/api/skin/{}", SKIN_API_URL, discord_id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API Error: {}", error_text));
    }

    Ok(())
}

/// Vérifie si un utilisateur a un skin custom
#[tauri::command]
pub async fn has_custom_skin(discord_id: String) -> Result<bool, String> {
    use reqwest::Client;
    use std::time::Duration;

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(&format!("{}/api/skin/{}", SKIN_API_URL, discord_id))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("API Error: {}", response.status()));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    result["hasCustomSkin"]
        .as_bool()
        .ok_or_else(|| "Invalid response format".to_string())
}

// ============================================================================
// Gestion du modèle de skin (default/slim)
// ============================================================================

/// Obtient le modèle de skin pour un utilisateur (default ou slim)
#[tauri::command]
pub async fn get_skin_model(
    discord_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let db_guard = state.db.lock().await;
    match db_guard.as_ref() {
        Some(db) => db.get_skin_model(&discord_id).await.map_err(|e| e.to_string()),
        None => Err("Base de données non connectée".to_string()),
    }
}

/// Met à jour le modèle de skin pour un utilisateur
#[tauri::command]
pub async fn update_skin_model(
    discord_id: String,
    model: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if model != "default" && model != "slim" {
        return Err("Modèle invalide. Utilisez 'default' ou 'slim'".to_string());
    }

    let db_guard = state.db.lock().await;
    match db_guard.as_ref() {
        Some(db) => db.update_skin_model(&discord_id, &model).await.map_err(|e| e.to_string()),
        None => Err("Base de données non connectée".to_string()),
    }
}

// ============================================================================
// News
// ============================================================================

/// Récupère les news publiées depuis le site admin (index.php?api=news).
/// Renvoie la liste triée la plus récente en premier (voir news.rs).
#[tauri::command]
pub async fn fetch_news() -> Result<Vec<NewsItem>, String> {
    crate::news::fetch_news().await
}