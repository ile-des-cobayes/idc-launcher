use crate::database::User;
use crate::discord_auth::DiscordUser;
use crate::news::NewsItem;
use crate::resources::{ResourceManager, SyncResult, FileInfo};
use crate::{database, AppState};
use serde::Serialize;
use std::collections::HashSet;
use tauri::{Emitter, Manager, State};

/// URL d'invitation vers le Discord officiel de L'île des Cobayes, utilisée
/// dans les messages d'erreur de vérification d'appartenance au serveur
/// (voir create_user et launch_game ci-dessous).
const DISCORD_INVITE_URL: &str = "https://discord.gg/KMBNxjnvxH";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchProgress {
    phase: String,
    progress: u8,
    label: String,
    detail: String,
}

fn emit_launch_progress(
    app: &tauri::AppHandle,
    phase: &str,
    progress: u8,
    label: &str,
    detail: impl Into<String>,
) {
    let update = LaunchProgress {
        phase: phase.to_string(),
        progress,
        label: label.to_string(),
        detail: detail.into(),
    };

    if let Err(error) = app.emit("launcher-progress", update) {
        eprintln!("Impossible d'envoyer la progression du launcher : {error}");
    }
}

/// Bascule la taille de la fenêtre principale entre le parcours de connexion
/// portrait et le hub du launcher. Cette commande tourne côté natif : elle
/// reste fiable même lorsqu'une implémentation de WebView refuse une demande
/// de redimensionnement provenant du JavaScript.
#[tauri::command]
pub fn set_launcher_window_mode(mode: String, app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Fenêtre principale introuvable".to_string())?;

    if mode == "hub" {
        // On se base sur la zone de travail (dock/barre des tâches exclue), et non
        // sur une taille fixe. Cela laisse toujours respirer le bureau, y compris
        // sur les écrans 13 pouces ou les configurations avec mise à l'échelle.
        let monitor = window
            .current_monitor()
            .map_err(|e| e.to_string())?
            .or(app.primary_monitor().map_err(|e| e.to_string())?);

        if let Some(monitor) = monitor {
            let work_area = monitor.work_area();
            let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;
            let available_width = work_area.size.width as f64 / scale_factor;
            let available_height = work_area.size.height as f64 / scale_factor;
            let width = (available_width * 0.86)
                .min(1280.0)
                .max(640.0)
                .min(available_width);
            let height = (available_height * 0.84)
                .min(760.0)
                .max(500.0)
                .min(available_height);

            window
                .set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
                    width.min(960.0),
                    height.min(620.0),
                ))))
                .map_err(|e| e.to_string())?;
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
                .map_err(|e| e.to_string())?;
        } else {
            // Cas exceptionnel (aucun moniteur exposé par l'OS) : un format
            // confortable mais plus compact que l'ancien 1360 × 800.
            window
                .set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
                    960.0, 620.0,
                ))))
                .map_err(|e| e.to_string())?;
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize::new(1280.0, 760.0)))
                .map_err(|e| e.to_string())?;
        }
    } else if mode == "login" {
        window
            .set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
                480.0, 720.0,
            ))))
            .map_err(|e| e.to_string())?;
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(480.0, 760.0)))
            .map_err(|e| e.to_string())?;
    } else {
        return Err("Mode de fenêtre inconnu".to_string());
    }

    window.center().map_err(|e| e.to_string())?;

    Ok(())
}

/// Synchronise le jeu, installe le profil Minecraft puis démarre Java. Chaque
/// jalon est envoyé au frontend via l'événement `launcher-progress`.
#[tauri::command]
pub async fn launch_game(
    username: String,
    discord_token: Option<String>,
    discord_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Si on n'a pas de token Discord, essayer de rafraîchir depuis le refresh token
    let discord_token = match discord_token {
        Some(token) => token,
        None => {
            // Essayer de rafraîchir le token automatiquement
            let discord_auth = state.discord_auth.clone();
            discord_auth
                .get_valid_access_token()
                .await
                .map_err(|_| "Token Discord expiré. Veuillez vous reconnecter.".to_string())?
        }
    };

    // Le compte peut exister mais son propriétaire avoir quitté (ou été
    // banni) du Discord depuis la création du compte, sans jamais repasser
    // par l'écran de connexion (le token peut être rafraîchi silencieusement) :
    // on revérifie donc l'appartenance au serveur à chaque lancement, pas
    // seulement à la création du compte (voir create_user plus bas).
    let in_guild = state
        .discord_auth
        .is_in_required_guild(&discord_token)
        .await
        .map_err(|e| format!("Impossible de vérifier ton appartenance au Discord : {e}"))?;

    if !in_guild {
        return Err(format!(
            "Tu dois être membre du Discord de L'île des Cobayes pour jouer. Rejoins-le ici : {DISCORD_INVITE_URL} — les sanctions Discord (bannissements) s'appliquent aussi au serveur Minecraft."
        ));
    }

    let resource_manager = state.resource_manager.clone();
    emit_launch_progress(
        &app,
        "preparing",
        6,
        "Préparation du launcher",
        "Vérification de ton installation",
    );
    emit_launch_progress(
        &app,
        "syncing",
        20,
        "Synchronisation de l'île",
        "Vérification des mods, configurations et ressources",
    );

    let sync_result = resource_manager
        .sync_game_resources()
        .await
        .map_err(|error| format!("Erreur de synchronisation des ressources : {error}"))?;

    if !sync_result.errors.is_empty() {
        let (path, error) = &sync_result.errors[0];
        return Err(format!(
            "Synchronisation incomplète ({} erreur(s)) : {} — {}",
            sync_result.errors.len(),
            path.display(),
            error
        ));
    }

    let changed_files = sync_result.downloaded.len() + sync_result.updated.len() + sync_result.deleted.len();
    let sync_detail = if changed_files == 0 {
        "Ton installation est déjà à jour.".to_string()
    } else {
        format!("{changed_files} fichier(s) mis à jour.")
    };
    emit_launch_progress(
        &app,
        "synced",
        64,
        "Installation synchronisée",
        sync_detail,
    );

    // Synchronisation des mods facultatifs
    emit_launch_progress(
        &app,
        "optional_mods",
        72,
        "Synchronisation des mods facultatifs",
        "Vérification de tes mods personnalisés",
    );

    // Récupérer les métadonnées des mods facultatifs
    let optional_mods_meta = crate::optional_mods::fetch_optional_mods_meta()
        .await
        .map_err(|e| format!("Impossible de récupérer les mods facultatifs : {}", e))?;

    // Déterminer quels mods sont activés pour ce joueur
    let db_guard = state.db.lock().await;
    let player_states = match db_guard.as_ref() {
        Some(db) => db
            .get_optional_mod_states(&discord_id)
            .await
            .map_err(|e| format!("Erreur base de données : {}", e))?,
        None => return Err("Base de données non connectée".to_string()),
    };

    // Construire la liste des IDs de mods activés
    let mut enabled_ids = HashSet::new();
    for meta in &optional_mods_meta {
        // Utiliser l'état personnalisé du joueur, ou la valeur par défaut si jamais touché
        let enabled = player_states
            .get(&meta.id)
            .copied()
            .unwrap_or(meta.enabled_by_default);
        if enabled {
            enabled_ids.insert(meta.id.clone());
        }
    }

    // Synchroniser les mods facultatifs
    let game_dir = ResourceManager::get_game_dir()?;
    let optional_sync_result = resource_manager
        .sync_optional_mods(&optional_mods_meta, &enabled_ids, &game_dir)
        .await
        .map_err(|error| format!("Erreur de synchronisation des mods facultatifs : {}", error))?;

    if !optional_sync_result.errors.is_empty() {
        let (path, error) = &optional_sync_result.errors[0];
        return Err(format!(
            "Synchronisation des mods facultatifs incomplète ({} erreur(s)) : {} — {}",
            optional_sync_result.errors.len(),
            path.display(),
            error
        ));
    }

    let optional_changed = optional_sync_result.downloaded.len() 
        + optional_sync_result.updated.len() 
        + optional_sync_result.deleted.len();
    let optional_detail = if optional_changed == 0 {
        "Tes mods facultatifs sont à jour.".to_string()
    } else {
        format!("{optional_changed} fichier(s) de mods facultatifs mis à jour.")
    };
    emit_launch_progress(
        &app,
        "optional_mods",
        78,
        "Mods facultatifs synchronisés",
        optional_detail,
    );

    let game_app = app.clone();
    let exit_app = app.clone();
    tokio::task::spawn_blocking(move || {
        crate::game::lancer_jeu_bloquant_avec_progress(
            Some(&discord_token),
            &username,
            move |phase, progress, label, detail| {
                emit_launch_progress(&game_app, phase, progress, label, detail);
            },
            move |_succes| {
                if let Err(error) = exit_app.emit("game-exited", ()) {
                    eprintln!("Impossible d'envoyer l'événement de fermeture du jeu : {error}");
                }
            },
        )
    })
        .await
        .map_err(|error| format!("Erreur interne lors du lancement : {error}"))?
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
/// ici, on récupère simplement le résultat déjà calculé (y compris le
/// flag `in_guild`, déjà vérifié à ce stade).
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

/// Rafraîchit le token Discord en utilisant le refresh token stocké
/// Retourne un nouveau DiscordUser avec le nouvel access_token
#[tauri::command]
pub async fn refresh_discord_token(state: State<'_, AppState>) -> Result<DiscordUser, String> {
    let discord_auth = state.discord_auth.clone();

    let access_token = discord_auth
        .get_valid_access_token()
        .await?;

    let mut user_info = discord_auth
        .get_user_info(&access_token)
        .await?;

    // Comme au login initial, on rafraîchit aussi l'appartenance au Discord
    // ici : ce résultat est utilisé par le frontend, et launch_game revérifie
    // de toute façon indépendamment côté backend avant de lancer le jeu.
    user_info.in_guild = discord_auth
        .is_in_required_guild(&access_token)
        .await
        .unwrap_or(false);

    Ok(DiscordUser {
        access_token: Some(access_token),
        ..user_info
    })
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

/// Crée le compte du joueur. Refusé si le joueur n'est pas membre du
/// Discord officiel (voir DISCORD_INVITE_URL) : `discord_token` est
/// revérifié ici plutôt que de faire confiance au flag `in_guild` déjà
/// calculé côté frontend, pour ne jamais dépendre uniquement d'un état
/// client potentiellement obsolète (le joueur a pu quitter le Discord
/// entre le login et la validation de son pseudo).
#[tauri::command]
pub async fn create_user(
    discord_id: String,
    username: String,
    discord_token: String,
    state: State<'_, AppState>,
) -> Result<User, String> {
    let in_guild = state
        .discord_auth
        .is_in_required_guild(&discord_token)
        .await
        .map_err(|e| format!("Impossible de vérifier ton appartenance au Discord : {e}"))?;

    if !in_guild {
        return Err(format!(
            "Tu dois d'abord rejoindre le Discord de L'île des Cobayes avant de créer ton compte : {DISCORD_INVITE_URL}"
        ));
    }

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

/// Base des covers de capes, servies par le panneau admin.
/// Contrairement à SKIN_API_URL, ce n'est pas embarqué via env!() car ce
/// n'est pas une donnée sensible ni amenée à changer par déploiement : c'est
/// une URL publique fixe du panneau admin.
const CAPE_COVER_BASE_URL: &str = "https://admin.ile-des-cobayes.fr/cape_covers";


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherCape {
    id: String,
    name: String,
    description: Option<String>,
    price: u64,
    purchasable: bool,
    texture_url: String,
    cover_url: Option<String>,
    owned: bool,
    selected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CosmeticsResponse {
    balance: u64,
    selected_cape_id: Option<String>,
    capes: Vec<LauncherCape>,
}

fn cosmetics_response(profile: database::CapeShopProfile) -> CosmeticsResponse {
    CosmeticsResponse {
        balance: profile.balance,
        selected_cape_id: profile.selected_cape_id,
        capes: profile
            .capes
            .into_iter()
            .map(|cape| {
                LauncherCape {
                    id: cape.id,
                    name: cape.name,
                    description: cape.description,
                    price: cape.price,
                    purchasable: cape.purchasable,
                    texture_url: cape.texture_url,
                    cover_url: cape.cover_url,
                    owned: cape.owned,
                    selected: cape.selected,
                }
            })
            .collect(),
    }
}

/// Récupère le portefeuille, le catalogue disponible et les capes possédées.
#[tauri::command]
pub async fn get_cape_shop(
    discord_id: String,
    state: State<'_, AppState>,
) -> Result<CosmeticsResponse, String> {
    let db_guard = state.db.lock().await;

    match db_guard.as_ref() {
        Some(db) => db
            .get_cape_shop_profile(&discord_id, SKIN_API_URL, CAPE_COVER_BASE_URL)
            .await
            .map(cosmetics_response),
        None => Err("Base de données non connectée".to_string()),
    }
}

/// Achète une cape. Le débit du portefeuille et l'ajout à la collection sont
/// réalisés dans la même transaction MySQL.
#[tauri::command]
pub async fn purchase_cape(
    discord_id: String,
    cape_id: String,
    state: State<'_, AppState>,
) -> Result<CosmeticsResponse, String> {
    let db_guard = state.db.lock().await;

    match db_guard.as_ref() {
        Some(db) => db
            .purchase_cape(&discord_id, &cape_id, SKIN_API_URL, CAPE_COVER_BASE_URL)
            .await
            .map(cosmetics_response),
        None => Err("Base de données non connectée".to_string()),
    }
}

/// Sélectionne une cape détenue, ou retire la cape active lorsque `cape_id`
/// est null. L'API de skins lira ce choix à la prochaine requête du mod.
#[tauri::command]
pub async fn select_cape(
    discord_id: String,
    cape_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CosmeticsResponse, String> {
    let db_guard = state.db.lock().await;

    match db_guard.as_ref() {
        Some(db) => db
            .select_cape(&discord_id, cape_id.as_deref(), SKIN_API_URL, CAPE_COVER_BASE_URL)
            .await
            .map(cosmetics_response),
        None => Err("Base de données non connectée".to_string()),
    }
}

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

// ============================================================================
// Mods facultatifs
// ============================================================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalModView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

/// Récupère la liste des mods facultatifs avec leur état (activé/désactivé) pour le joueur.
/// L'état par défaut (enabled_by_default) est utilisé si le joueur n'a jamais explicitement
/// changé l'état du mod.
#[tauri::command]
pub async fn get_optional_mods(
    discord_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<OptionalModView>, String> {
    // Récupérer les métadonnées des mods depuis l'API
    let mods_meta = crate::optional_mods::fetch_optional_mods_meta()
        .await
        .map_err(|e| format!("Impossible de récupérer les mods facultatifs : {}", e))?;

    // Récupérer les états personnalisés du joueur depuis la base de données
    let db_guard = state.db.lock().await;
    let player_states = match db_guard.as_ref() {
        Some(db) => db
            .get_optional_mod_states(&discord_id)
            .await
            .map_err(|e| format!("Erreur base de données : {}", e))?,
        None => return Err("Base de données non connectée".to_string()),
    };

    // Construire la réponse en croisant métadonnées et états joueur
    let mut result = Vec::new();
    for meta in mods_meta {
        // Vérifier si le joueur a un état personnalisé pour ce mod
        let enabled = player_states
            .get(&meta.id)
            .copied()
            .unwrap_or(meta.enabled_by_default);

        result.push(OptionalModView {
            id: meta.id,
            name: meta.name,
            description: meta.description,
            enabled,
        });
    }

    Ok(result)
}

/// Met à jour l'état activé/désactivé d'un mod facultatif pour le joueur.
#[tauri::command]
pub async fn set_optional_mod_enabled(
    discord_id: String,
    mod_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().await;
    match db_guard.as_ref() {
        Some(db) => db
            .set_optional_mod_enabled(&discord_id, &mod_id, enabled)
            .await
            .map_err(|e| format!("Erreur base de données : {}", e)),
        None => Err("Base de données non connectée".to_string()),
    }
}