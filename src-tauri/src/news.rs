use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Une news publiée depuis le site admin. Les champs collent exactement à
/// ce que renvoie `index.php?api=news` côté admin (voir index.php).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub excerpt: String,
    pub content: String,
    pub cover_url: Option<String>,
    pub created_at: Option<String>,
}

// URL de l'API news, embarquée à la COMPILATION via env!(), exactement comme
// SKIN_API_URL dans commands.rs (voir le commentaire associé là-bas pour le
// pourquoi : std::env::var() retomberait silencieusement sur rien en prod).
//
// Ajoute dans ton .env, par exemple :
// NEWS_API_URL=https://ouepamal.fr/admin/index.php?api=news
// (adapte le chemin selon l'endroit où le panneau admin est déployé — c'est
// le même index.php que celui qui gère les ressources, avec ?api=news)
//
// build.rs forwarde déjà TOUTES les clés du .env automatiquement, donc
// aucune modification de build.rs n'est nécessaire pour cette variable.
const NEWS_API_URL: &str = env!("NEWS_API_URL");

/// Récupère la liste des news publiées, la plus récente en premier.
/// Le tri est déjà fait côté serveur (saveNews() dans index.php), on ne
/// re-trie pas ici.
pub async fn fetch_news() -> Result<Vec<NewsItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(NEWS_API_URL)
        .send()
        .await
        .map_err(|e| format!("Impossible de contacter le serveur de news : {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Erreur HTTP {} lors de la récupération des news",
            response.status()
        ));
    }

    response
        .json::<Vec<NewsItem>>()
        .await
        .map_err(|e| format!("Erreur de lecture des news : {e}"))
}