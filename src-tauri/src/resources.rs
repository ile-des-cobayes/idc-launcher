use reqwest::Client;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Write, Read};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Configuration des URLs du serveur de ressources
pub struct ResourceConfig {
    pub base_url: String,
    pub server_base_path: String,    // Dossier à installer une seule fois (si absent)
    pub server_updates_path: String, // Dossier à vérifier à chaque lancement
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            base_url: env!("RESOURCES_SERVER").to_string(),
            server_base_path: "/base".to_string(),
            server_updates_path: "/updates".to_string(),
        }
    }
}

/// Représente un fichier avec son chemin relatif et son hash SHA-256
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub hash: String,
    pub size: u64,
}

/// État de synchronisation d'un fichier
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Le fichier existe localement et correspond au serveur
    UpToDate,
    /// Le fichier n'existe pas localement
    Missing,
    /// Le fichier existe mais diffère (hash différent ou taille différente)
    Modified,
    /// Le fichier existe localement mais pas sur le serveur (à supprimer)
    Extra,
}

/// Résultat d'une synchronisation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub downloaded: Vec<PathBuf>,
    pub updated: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
}

impl SyncResult {
    pub fn new() -> Self {
        Self {
            downloaded: Vec::new(),
            updated: Vec::new(),
            deleted: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Gestionnaire des ressources du jeu
pub struct ResourceManager {
    config: ResourceConfig,
    client: Client,
}

impl ResourceManager {
    pub fn new(config: ResourceConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Impossible de créer le client HTTP : {}", e))?;

        Ok(Self {
            config,
            client,
        })
    }

    /// Obtient le dossier de jeu (idc-launcher)
    pub fn get_game_dir() -> Result<PathBuf, String> {
        #[cfg(target_os = "linux")]
        let base = dirs::home_dir();

        #[cfg(not(target_os = "linux"))]
        let base = dirs::config_dir();

        let base = base.ok_or_else(|| "Impossible de déterminer le dossier utilisateur".to_string())?;
        Ok(base.join("idc-launcher"))
    }

    /// Construit l'URL complète pour un chemin donné
    fn build_url(&self, path: &str) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        let p = path.trim_start_matches('/');
        format!("{}/{}", base, p)
    }

    /// Télécharge le contenu d'un fichier depuis le serveur
    async fn download_file_content(&self, remote_path: &str) -> Result<Vec<u8>, String> {
        let url = self.build_url(remote_path);

        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| format!("Erreur de téléchargement pour {} : {}", url, e))?;

        if !response.status().is_success() {
            return Err(format!("Erreur HTTP {} pour {}", response.status(), url));
        }

        let bytes = response.bytes()
            .await
            .map_err(|e| format!("Erreur de lecture de la réponse pour {} : {}", url, e))?;

        Ok(bytes.to_vec())
    }

    /// Calcul le hash SHA-256 d'un fichier local
    fn compute_file_hash(path: &Path) -> Result<String, String> {
        let mut file = File::open(path)
            .map_err(|e| format!("Impossible d'ouvrir {} : {}", path.display(), e))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| format!("Erreur de lecture de {} : {}", path.display(), e))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Obtient la liste des fichiers sur le serveur avec leurs infos
    pub async fn fetch_remote_files(&self, server_path: &str) -> Result<HashMap<String, FileInfo>, String> {
        // On suppose que le serveur fournit un manifest.json (généré par le panneau d'administration)
        // Si le chemin se termine par /, on utilise manifest.json, sinon on l'ajoute
        let clean_path = server_path.trim_end_matches('/');
        let manifest_url = self.build_url(&format!("{}/manifest.json", clean_path));

        let response = self.client.get(&manifest_url)
            .send()
            .await
            .map_err(|e| format!("Erreur de téléchargement du manifest pour {} : {}", manifest_url, e))?;

        if response.status().is_success() {
            let manifest: HashMap<String, FileInfo> = response.json()
                .await
                .map_err(|e| format!("Erreur de parsing du manifest pour {} : {}", manifest_url, e))?;
            return Ok(manifest);
        }

        // Pour la rétrocompatibilité, on essaie aussi avec .index.json
        let index_url = self.build_url(&format!("{}/.index.json", clean_path));
        let response = self.client.get(&index_url)
            .send()
            .await
            .map_err(|e| format!("Erreur de téléchargement de l'index pour {} : {}", index_url, e))?;

        if response.status().is_success() {
            let index: HashMap<String, FileInfo> = response.json()
                .await
                .map_err(|e| format!("Erreur de parsing de l'index pour {} : {}", index_url, e))?;
            return Ok(index);
        }

        // Si aucun manifest n'est trouvé
        Err(format!("Aucun manifest trouvé à {}", manifest_url))
    }

    /// Scanne récursivement un dossier local
    fn scan_local_dir(local_path: &Path, base_path: &Path) -> Result<HashMap<String, FileInfo>, String> {
        let mut files = HashMap::new();

        for entry in walkdir::WalkDir::new(local_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let relative_path = entry.path()
                    .strip_prefix(base_path)
                    .map_err(|_| "Impossible de calculer le chemin relatif".to_string())?
                    .to_path_buf();

                let path_str = relative_path.to_str()
                    .ok_or_else(|| "Chemin invalide".to_string())?
                    .replace('\\', "/");

                let hash = Self::compute_file_hash(entry.path())?;
                let size = entry.metadata()
                    .map(|m| m.len())
                    .map_err(|e| format!("Impossible de lire la taille de {} : {}", entry.path().display(), e))?;

                files.insert(path_str, FileInfo {
                    path: relative_path,
                    hash,
                    size,
                });
            }
        }

        Ok(files)
    }

    /// Calcule l'ensemble des dossiers racines "gérés" par un manifest
    /// (ex: "mods", "config", "resourcepacks"...), à partir du premier
    /// segment de chaque chemin relatif listé.
    ///
    /// Sert à restreindre les scans locaux (et donc la détection de fichiers
    /// "Extra" à supprimer) à ces seuls dossiers : le reste du dossier de jeu
    /// (saves, screenshots, logs, options.txt...) n'est jamais parcouru ni
    /// touché, même s'il n'apparaît dans aucun manifest.
    fn managed_root_dirs(remote_files: &HashMap<String, FileInfo>) -> std::collections::HashSet<String> {
        remote_files
            .keys()
            .filter_map(|p| p.split('/').next().map(|s| s.to_string()))
            .collect()
    }

    /// Compare les fichiers locaux et distants pour un manifest donné.
    ///
    /// - `check_modified` : si `false`, un fichier présent localement est
    ///   toujours considéré `UpToDate`, quel que soit son contenu (flux
    ///   "base" : on installe si absent, on ne vérifie jamais l'intégrité).
    ///   Si `true`, un hash/taille différent est marqué `Modified` (flux
    ///   "updates" : on restaure les fichiers altérés).
    /// - `check_extra` : si `true`, on détecte aussi les fichiers locaux à
    ///   supprimer, mais UNIQUEMENT à l'intérieur des dossiers gérés par ce
    ///   manifest (voir `managed_root_dirs`) — jamais sur tout `local_base`,
    ///   pour ne jamais toucher un dossier non mentionné comme "saves".
    async fn compare_files(
        &self,
        server_path: &str,
        local_base: &Path,
        check_modified: bool,
        check_extra: bool,
    ) -> Result<HashMap<String, SyncStatus>, String> {
        let remote_files = self.fetch_remote_files(server_path).await?;

        let mut status = HashMap::new();

        // On ne vérifie QUE les fichiers listés dans le manifest : aucun scan
        // du dossier de jeu entier ici.
        for (remote_path, remote_info) in &remote_files {
            let local_path = local_base.join(remote_path);

            if !local_path.exists() {
                status.insert(remote_path.clone(), SyncStatus::Missing);
                continue;
            }

            if check_modified {
                let local_hash = Self::compute_file_hash(&local_path)?;
                let local_size = fs::metadata(&local_path)
                    .map(|m| m.len())
                    .map_err(|e| format!("Impossible de lire {} : {}", local_path.display(), e))?;

                if local_hash == remote_info.hash && local_size == remote_info.size {
                    status.insert(remote_path.clone(), SyncStatus::UpToDate);
                } else {
                    status.insert(remote_path.clone(), SyncStatus::Modified);
                }
            } else {
                status.insert(remote_path.clone(), SyncStatus::UpToDate);
            }
        }

        
        if check_extra {
            // On scanne uniquement les dossiers racines présents dans CE
            // manifest, jamais local_base dans son intégralité.
            let managed_roots = Self::managed_root_dirs(&remote_files);

            for root in managed_roots {
                let root_path = local_base.join(&root);
                if !root_path.exists() {
                    continue;
                }

                let local_files = Self::scan_local_dir(&root_path, local_base)?;

                for (local_path, _) in &local_files {
                    if !remote_files.contains_key(local_path) {
                        status.insert(local_path.clone(), SyncStatus::Extra);
                    }
                }
            }
        }

        Ok(status)
    }

    /// Télécharge un fichier depuis le serveur vers le dossier local
    /// Les fichiers sont placés directement dans local_base (sans sous-dossier base/updates)
    async fn download_file(
        &self,
        _server_path: &str,
        local_base: &Path,
        relative_path: &str,
    ) -> Result<(), String> {
        // _server_path n'est plus utilisé pour le chemin local
        // On utilise uniquement relative_path qui contient déjà le chemin complet
        let full_server_path = format!("{}/{}", _server_path.trim_end_matches('/'), relative_path);
        let content = self.download_file_content(&full_server_path).await?;

        // Le fichier est placé directement dans local_base (idc-launcher/)
        let file_path = local_base.join(relative_path);

        // Créer les dossiers parents si nécessaire
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Impossible de créer {} : {}", parent.display(), e))?;
            }
        }

        // Écrire le fichier
        let mut file = File::create(&file_path)
            .map_err(|e| format!("Impossible de créer {} : {}", file_path.display(), e))?;

        file.write_all(&content)
            .map_err(|e| format!("Impossible d'écrire {} : {}", file_path.display(), e))?;

        Ok(())
    }

    /// Supprime un fichier local
    fn delete_local_file(local_base: &Path, relative_path: &str, _server_path: &str) -> Result<(), String> {
        // Le fichier est directement dans local_base (sans sous-dossier server_path)
        let file_path = local_base.join(relative_path);

        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| format!("Impossible de supprimer {} : {}", file_path.display(), e))?;
        }

        // Supprimer les dossiers vides parents
        Self::cleanup_empty_dirs(&file_path);

        Ok(())
    }

    /// Nettoie les dossiers vides récursivement
    fn cleanup_empty_dirs(path: &Path) {
        let mut current = path.parent();

        while let Some(dir) = current {
            if dir.exists() && dir.is_dir() {
                if dir.read_dir().map_or(true, |mut rd| rd.next().is_none()) {
                    let _ = fs::remove_dir(dir);
                } else {
                    break;
                }
            } else {
                break;
            }

            current = dir.parent();
        }
    }

    /// Synchronise un dossier serveur vers le dossier local
    /// - `force_install` : si true, installe tout en une fois quand rien n'existe encore
    /// - `check_modified` : si true, un fichier altéré est re-téléchargé (flux "updates").
    ///   Si false, un fichier présent n'est jamais revérifié (flux "base").
    /// - `check_extra` : si true, les fichiers locaux hors manifest (dans les dossiers
    ///   gérés par ce manifest) sont supprimés (flux "updates"). Toujours false pour "base".
    pub async fn sync_directory(
        &self,
        server_path: &str,
        local_base: &Path,
        force_install: bool,
        check_modified: bool,
        check_extra: bool,
    ) -> Result<SyncResult, String> {
        let mut result = SyncResult::new();
        let status = self.compare_files(server_path, local_base, check_modified, check_extra).await?;

        // Si c'est le dossier base et qu'il n'existe pas du tout, on l'installe
        // On vérifie si le dossier base n'a jamais été installé
        if force_install {
            // Vérifier si au moins un fichier du manifest existe localement
            let remote_files = self.fetch_remote_files(server_path).await?;
            let any_file_exists = remote_files.keys().any(|path| local_base.join(path).exists());

            if !any_file_exists {
                // Aucun fichier n'existe, installer tout
                for (relative_path, _) in &remote_files {
                    match self.download_file(server_path, local_base, relative_path).await {
                        Ok(_) => {
                            result.downloaded.push(PathBuf::from(relative_path));
                        }
                        Err(e) => {
                            result.errors.push((PathBuf::from(relative_path), e));
                        }
                    }
                }

                return Ok(result);
            }
        }

        // Synchronisation normale
        for (relative_path, sync_status) in &status {
            match sync_status {
                SyncStatus::Missing | SyncStatus::Modified => {
                    match self.download_file(server_path, local_base, relative_path).await {
                        Ok(_) => {
                            if *sync_status == SyncStatus::Missing {
                                result.downloaded.push(PathBuf::from(relative_path));
                            } else {
                                result.updated.push(PathBuf::from(relative_path));
                            }
                        }
                        Err(e) => {
                            result.errors.push((PathBuf::from(relative_path), e));
                        }
                    }
                }
                SyncStatus::Extra => {
                    match Self::delete_local_file(local_base, relative_path, server_path) {
                        Ok(_) => {
                            result.deleted.push(PathBuf::from(relative_path));
                        }
                        Err(e) => {
                            result.errors.push((PathBuf::from(relative_path), e));
                        }
                    }
                }
                SyncStatus::UpToDate => {}
            }
        }

        Ok(result)
    }

    /// Synchronise les ressources du jeu
    /// - Installe le dossier base s'il manque
    /// - Synchronise toujours le dossier updates
    pub async fn sync_game_resources(&self) -> Result<SyncResult, String> {
        let game_dir = Self::get_game_dir()?;

        // Créer le dossier de jeu s'il n'existe pas
        if !game_dir.exists() {
            fs::create_dir_all(&game_dir)
                .map_err(|e| format!("Impossible de créer {} : {}", game_dir.display(), e))?;
        }

        let mut total_result = SyncResult::new();

        // 1. Synchroniser le dossier base : installation des fichiers manquants
        // uniquement. Pas de vérification d'intégrité (check_modified=false) et
        // pas de suppression (check_extra=false) : un fichier "base" modifié par
        // le joueur ou un dossier non mentionné (saves, etc.) n'est jamais touché.
        let base_result = self.sync_directory(
            &self.config.server_base_path,
            &game_dir,
            true,  // force install si rien n'existe encore
            false, // check_modified
            false, // check_extra
        ).await?;

        total_result.downloaded.extend(base_result.downloaded);
        total_result.updated.extend(base_result.updated);
        total_result.deleted.extend(base_result.deleted);
        total_result.errors.extend(base_result.errors);

        // 2. Synchroniser le dossier updates (toujours) : restaure les fichiers
        // manquants ou altérés, supprime les fichiers retirés du manifest — mais
        // uniquement à l'intérieur des dossiers gérés par ce manifest (voir
        // managed_root_dirs), jamais sur le reste du dossier de jeu.
        let updates_result = self.sync_directory(
            &self.config.server_updates_path,
            &game_dir,
            false, // synchronisation normale
            true,  // check_modified
            true,  // check_extra
        ).await?;

        total_result.downloaded.extend(updates_result.downloaded);
        total_result.updated.extend(updates_result.updated);
        total_result.deleted.extend(updates_result.deleted);
        total_result.errors.extend(updates_result.errors);

        Ok(total_result)
    }

    /// Vérifie si les ressources sont à jour
    pub async fn check_resources_up_to_date(&self) -> Result<bool, String> {
        let game_dir = Self::get_game_dir()?;

        if !game_dir.exists() {
            return Ok(false);
        }

        // Vérifier les updates (intégrité + fichiers en trop dans les dossiers gérés)
        let status = self.compare_files(&self.config.server_updates_path, &game_dir, true, true).await?;

        // Si tous les fichiers sont UpToDate, c'est bon
        let all_updated = !status.values().any(|s| *s != SyncStatus::UpToDate);

        // Vérifier aussi que le dossier base a au moins un fichier installé
        // (on ne vérifie plus l'existence du dossier, mais celle d'au moins un fichier du manifest base)
        let base_files = self.fetch_remote_files(&self.config.server_base_path).await?;
        let base_exists = base_files.keys().any(|path| game_dir.join(path).exists());

        Ok(all_updated && base_exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compute_hash() {
        // Test simple du calcul de hash
        let test_content = b"test content";
        let mut hasher = Sha256::new();
        hasher.update(test_content);
        let expected_hash = format!("{:x}", hasher.finalize());

        // Créer un fichier temporaire
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, test_content).unwrap();

        let computed_hash = ResourceManager::compute_file_hash(&test_file).unwrap();
        assert_eq!(computed_hash, expected_hash);
    }
}