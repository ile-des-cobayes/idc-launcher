use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// État du launcher pour le Discord RPC
#[derive(Debug, Clone, PartialEq)]
pub enum RpcState {
    /// Dans le launcher (menu principal, news, etc.)
    InLauncher,
    /// En jeu avec timer
    InGame { start_time: std::time::SystemTime },
}

/// Gestionnaire de la présence Discord Rich Presence
pub struct DiscordRpc {
    client: Arc<Mutex<Option<DiscordIpcClient>>>,  
    client_id: String,
    current_state: Arc<Mutex<RpcState>>,
}

impl DiscordRpc {
    /// Crée une nouvelle instance du gestionnaire Discord RPC
    pub fn new() -> Self {
        // Récupère le client ID depuis les variables d'environnement (injection via build.rs)
        let client_id = env!("DISCORD_CLIENT_ID").to_string();
        
        Self {
            client: Arc::new(Mutex::new(None)),
            client_id,
            current_state: Arc::new(Mutex::new(RpcState::InLauncher)),
        }
    }

    /// Initialise la connexion avec Discord IPC
    pub async fn connect(&self) -> Result<(), String> {
        let mut client_guard = self.client.lock().await;
        
        if client_guard.is_some() {
            return Ok(());
        }

        let mut client = DiscordIpcClient::new(&self.client_id);
        
        // Connexion au client Discord
        client.connect()
            .map_err(|e| format!("Impossible de se connecter à Discord IPC: {}", e))?;

        *client_guard = Some(client);
        drop(client_guard);
        
        // Mettre à jour la présence initiale
        self.update_presence().await?;
        
        Ok(())
    }

    /// Met à jour l'état actuel
    pub async fn set_state(&self, state: RpcState) -> Result<(), String> {
        let mut current = self.current_state.lock().await;
        *current = state.clone();
        drop(current);
        
        // Mettre à jour la présence
        self.update_presence().await
    }

    /// Met à jour la présence Discord en fonction de l'état actuel
    async fn update_presence(&self) -> Result<(), String> {
        let state = self.current_state.lock().await.clone();
        let mut client_guard = self.client.lock().await;
        
        let client = client_guard.as_mut().ok_or("Client Discord non connecté")?;
        
        let activity = match state {
            RpcState::InLauncher => {
                Activity::new()
                    .state("Dans le launcher")
                    .details("L'île des Cobayes")
                    .assets(
                        Assets::new()
                            .large_image("idc_logo")
                            .large_text("L'île des Cobayes")
                    )
            }
            RpcState::InGame { start_time } => {
                let start_timestamp = start_time
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "Impossible de calculer le timestamp")?;
                
                Activity::new()
                    .state("En jeu")
                    .details("L'île des Cobayes")
                    .timestamps(
                        Timestamps::new()
                            .start(start_timestamp.as_secs() as i64)
                    )
                    .assets(
                        Assets::new()
                            .large_image("idc_logo")
                            .large_text("L'île des Cobayes")
                    )
            }
        };

        client.set_activity(activity)
            .map_err(|e| format!("Impossible de mettre à jour la présence Discord: {}", e))?;
        
        Ok(())
    }

    /// Indique que l'utilisateur est en jeu (démarre le timer)
    pub async fn set_in_game(&self) -> Result<(), String> {
        self.set_state(RpcState::InGame {
            start_time: std::time::SystemTime::now()
        }).await
    }

    /// Indique que l'utilisateur est dans le launcher
    pub async fn set_in_launcher(&self) -> Result<(), String> {
        self.set_state(RpcState::InLauncher).await
    }

    /// Déconnecte le client Discord IPC
    pub async fn disconnect(&self) {
        let mut client_guard = self.client.lock().await;
        if let Some(ref mut client) = client_guard.as_mut() {
            let _ = client.close();
        }
        *client_guard = None;
    }
}

impl Clone for DiscordRpc {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            client_id: self.client_id.clone(),
            current_state: self.current_state.clone(),
        }
    }
}
