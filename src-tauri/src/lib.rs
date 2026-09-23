mod database;
mod discord_auth;
mod callback_server;
mod game;
mod commands;
mod resources;
mod news;
mod optional_mods;
mod discord_rpc;

use database::Database;
use discord_auth::DiscordAuth;
use callback_server::CallbackServer;
use resources::ResourceManager;
use discord_rpc::DiscordRpc;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

/// État partagé entre toutes les commandes Tauri.
/// `db` est `None` tant que la connexion asynchrone n'est pas terminée
/// (voir le hook `.setup()` plus bas).
///
/// `discord_auth` est dans un `Arc` car il doit maintenant être partagé
/// avec le serveur de callback : ce dernier échange lui-même le code
/// contre un token dès qu'il arrive, pour pouvoir afficher le pseudo et
/// l'avatar directement sur la page de confirmation dans le navigateur.
///
/// `resource_manager` gère le téléchargement et la synchronisation des
/// ressources du jeu (mods, resource packs, configs, etc.)
///
/// `discord_rpc` gère la présence Discord Rich Presence
pub struct AppState {
    pub db: Mutex<Option<Database>>,
    pub discord_auth: Arc<DiscordAuth>,
    pub callback_server: CallbackServer,
    pub resource_manager: Arc<ResourceManager>,
    pub discord_rpc: Arc<DiscordRpc>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Plus besoin de dotenv::dotenv().ok() ici : les valeurs du .env sont
    // maintenant embarquées au moment de la compilation (voir build.rs +
    // discord_auth.rs / database.rs), donc disponibles au runtime sans
    // dépendre d'un fichier .env présent à côté du binaire.

    // Initialiser le gestionnaire de ressources avec la configuration par défaut
    // (les URLs peuvent être modifiées via des variables d'environnement ou une config externe)
    let resource_config = resources::ResourceConfig {
        base_url: std::env::var("RESOURCE_SERVER_URL")
            .unwrap_or_else(|_| env!("RESOURCES_SERVER").to_string()),
        server_base_path: std::env::var("RESOURCE_BASE_PATH")
            .unwrap_or_else(|_| "/base".to_string()),
        server_updates_path: std::env::var("RESOURCE_UPDATES_PATH")
            .unwrap_or_else(|_| "/updates".to_string()),
    };

    let resource_manager = Arc::new(
        ResourceManager::new(resource_config)
            .expect("Impossible d'initialiser le gestionnaire de ressources")
    );

    // Initialiser le Discord RPC
    let discord_rpc = Arc::new(DiscordRpc::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            db: Mutex::new(None),
            discord_auth: Arc::new(DiscordAuth::new()),
            callback_server: CallbackServer::new(),
            resource_manager,
            discord_rpc: discord_rpc.clone(),
        })
        .setup(move |app| {
            // Le launcher commence sur l'écran de connexion, volontairement
            // portrait. Le frontend agrandit ensuite la même fenêtre pour le
            // hub de jeu, sans jamais la mettre en plein écran.
            if let Some(window) = app.get_webview_window("main") {
                window.set_title("L'île des Cobayes")?;
                window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
                    480.0, 720.0,
                ))))?;
            }

            let handle = app.handle().clone();
            let discord_rpc_clone = discord_rpc.clone();

            // Connexion initiale au Discord RPC (dans le launcher)
            tauri::async_runtime::spawn(async move {
                let rpc = discord_rpc_clone.as_ref().clone();
                if let Err(e) = rpc.connect().await {
                    eprintln!("Erreur de connexion Discord RPC : {}", e);
                }
            });

            // La connexion à MySQL est async : on la lance en tâche de fond
            // et on remplit `AppState.db` une fois prête, sans bloquer le
            // démarrage de la fenêtre.
            tauri::async_runtime::spawn(async move {
                match Database::new().await {
                    Ok(database) => {
                        if let Err(e) = database.create_tables().await {
                            eprintln!("Erreur lors de la création des tables : {e}");
                        }
                        let state = handle.state::<AppState>();
                        *state.db.lock().await = Some(database);
                        println!("Base de données connectée.");
                    }
                    Err(e) => {
                        eprintln!("Impossible de se connecter à la base de données : {e}");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_launcher_window_mode,
            commands::start_discord_auth,
            commands::complete_discord_auth,
            commands::refresh_discord_token,
            commands::get_user_by_discord_id,
            commands::create_user,
            commands::launch_game,
            commands::sync_resources,
            commands::check_resources_up_to_date,
            commands::list_server_files,
            commands::get_game_directory,
            commands::upload_skin,
            commands::delete_skin,
            commands::has_custom_skin,
            commands::get_skin_model,
            commands::update_skin_model,
            commands::get_cape_shop,
            commands::purchase_cape,
            commands::select_cape,
            commands::fetch_news,
            commands::get_optional_mods,
            commands::set_optional_mod_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

