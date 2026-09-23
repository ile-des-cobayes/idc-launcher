use serde::{Deserialize, Serialize};
use std::time::Duration;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionalModMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub enabled_by_default: bool,
    pub server_path: String,
    pub updated_at: Option<String>,
}

const OPTIONAL_MODS_API_URL: &str = env!("OPTIONAL_MODS_API_URL");

/// Récupère la liste des mods facultatifs publiés, avec leurs métadonnées.
///
pub async fn fetch_optional_mods_meta() -> Result<Vec<OptionalModMeta>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(OPTIONAL_MODS_API_URL)
        .send()
        .await
        .map_err(|e| format!("Impossible de contacter le serveur des mods facultatifs : {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Erreur HTTP {} lors de la récupération des mods facultatifs",
            response.status()
        ));
    }

    response
        .json::<Vec<OptionalModMeta>>()
        .await
        .map_err(|e| format!("Erreur de lecture des mods facultatifs : {e}"))
}
