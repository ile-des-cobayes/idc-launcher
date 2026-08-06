use portablemc::forge::{self, Loader, Version as ForgeVersion};
use std::path::PathBuf;

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

/// Prépare puis démarre Minecraft. Les étapes sont remontées au launcher par
/// le callback : portablemc ne fournit pas de progression fine pendant son
/// installation, on expose donc uniquement des jalons qui correspondent à de
/// vraies opérations du processus.
pub fn lancer_jeu_bloquant_avec_progress<F>(pseudo: &str, on_progress: F) -> Result<(), String>
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

    on_progress(
        "neoforge",
        82,
        "Installation de NeoForge",
        "Préparation des bibliothèques et du profil de jeu",
    );
    let mut installer = forge::Installer::new(Loader::NeoForge, ForgeVersion::Name("21.1.232".to_string()));

    {
        let mojang = installer.mojang_mut();
        mojang.set_version("1.21.1");
        mojang.set_auth_offline_username(pseudo);
        mojang.base_mut().set_mc_dir(dossier);
    }

    let game = installer
        .install(())
        .map_err(|e| format!("Erreur lors de l'installation : {e}"))?;

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
