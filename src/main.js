const { invoke } = window.__TAURI__.core;
const { openUrl } = window.__TAURI__.opener;
const tauriWindow = window.__TAURI__.window;

const ecranConnexion = document.getElementById("ecran-connexion");
const ecranPseudo = document.getElementById("ecran-pseudo");
const ecranConnecte = document.getElementById("ecran-connecte");
const statutAuth = document.getElementById("statut-auth");
const erreur = document.getElementById("erreur");
const avatarInitiale = document.getElementById("avatar-initiale");

const btnDiscord = document.getElementById("btn-discord");
const btnValiderPseudo = document.getElementById("btn-valider-pseudo");
const btnJouer = document.getElementById("btn-jouer");

// Gestion du skin
const btnProfil = document.getElementById("btn-profil");
const avatarSkin = document.getElementById("avatar-skin");
const modalSkin = document.getElementById("modal-skin");
const skinPreviewImg = document.getElementById("skin-preview-img");
const inputSkinUpload = document.getElementById("input-skin-upload");
const btnDeleteSkin = document.getElementById("btn-delete-skin");
const btnCloseModal = document.getElementById("btn-close-modal");
const skinStatus = document.getElementById("skin-status");
const modelSteve = document.getElementById("model-steve");
const modelAlex = document.getElementById("model-alex");

// Gestion des news / notifications
const btnNavNews = document.getElementById("btn-nav-news");
const btnNavAccueil = document.getElementById("btn-nav-accueil");
const modalNews = document.getElementById("modal-news");
const newsListeEl = document.getElementById("news-liste");
const btnCloseNewsModal = document.getElementById("btn-close-news-modal");
const apercuNews = document.getElementById("apercu-news");
const apercuNewsCover = document.getElementById("apercu-news-cover");
const apercuNewsTitre = document.getElementById("apercu-news-titre");
const apercuNewsExtrait = document.getElementById("apercu-news-extrait");

// Éléments pour les onglets
const ongletAccueil = document.getElementById("onglet-accueil");
const ongletNews = document.getElementById("onglet-news");
const newsListeOngletEl = document.getElementById("news-liste-onglet");
const btnNotifications = document.getElementById("btn-notifications");
const notifDropdown = document.getElementById("notif-dropdown");
const notifListeEl = document.getElementById("notif-liste");
const notifDot = document.getElementById("notif-dot");

let discordUserCourant = null;
let usernameCourant = null;
let skinModel = "default";
let newsCourantes = [];

// Ecrans de connexion/pseudo : format vertical.
// Ecran de jeu : format fenetre classique, comme un launcher normal.
const TAILLE_VERTICALE = { largeur: 440, hauteur: 680 };
const TAILLE_JEU = { largeur: 1310, hauteur: 690 };

// Clé locale (persistée entre lancements) qui retient l'id de la dernière
// news déjà vue par ce joueur, pour savoir s'il faut afficher le point
// rouge de notification.
const CLE_DERNIERE_NEWS_VUE = "idc_derniere_news_vue";

async function definirTailleFenetre(largeur, hauteur) {
  try {
    if (!tauriWindow) return;
    const fenetre = tauriWindow.getCurrentWindow();
    await fenetre.setSize(new tauriWindow.LogicalSize(largeur, hauteur));
    await fenetre.center();
  } catch (e) {
    console.warn("Impossible de redimensionner la fenetre :", e);
  }
}

function afficherEcran(ecran) {
  [ecranConnexion, ecranPseudo, ecranConnecte].forEach((e) => e.classList.add("cache"));
  ecran.classList.remove("cache");

  if (ecran === ecranConnecte) {
    definirTailleFenetre(TAILLE_JEU.largeur, TAILLE_JEU.hauteur);
  } else {
    definirTailleFenetre(TAILLE_VERTICALE.largeur, TAILLE_VERTICALE.hauteur);
  }
}

function afficherErreur(message) {
  erreur.textContent = message;
  erreur.classList.remove("cache");
}

// ============================================================================
// Gestion du Skin
// ============================================================================

// Extrait la tête d'un skin Minecraft (8x8 pixels -> 40x40)
async function extractHeadFromSkin(skinUrl) {
  return new Promise((resolve) => {
    const img = new Image();
    // Indispensable : sans ça, le canvas est "tainted" par une image
    // cross-origin (le skin vient de ouepamal.fr, pas de l'origine de
    // l'app) et canvas.toDataURL() lève une SecurityError silencieuse
    // plus bas, qui empêchait la Promise de se résoudre.
    // Nécessite que le serveur d'images renvoie un header
    // Access-Control-Allow-Origin (sinon on retombe sur le catch ci-dessous).
    img.crossOrigin = "anonymous";
    const timeout = setTimeout(() => resolve(null), 3000); // Timeout après 3 secondes

    img.onload = () => {
      clearTimeout(timeout);
      try {
        const canvas = document.createElement("canvas");
        canvas.width = 40;
        canvas.height = 40;
        const ctx = canvas.getContext("2d");
        ctx.imageSmoothingEnabled = false;

        // Le visage (face avant de la tête) est à (8,8)-(16,16) dans un
        // skin 64x64. (8,0)-(16,8) est le DESSUS du crâne, pas le visage.
        ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 40, 40);

        resolve(canvas.toDataURL("image/png"));
      } catch (e) {
        // Canvas tainted (CORS) ou autre erreur : on abandonne l'extraction,
        // l'appelant retombera sur l'URL du skin complet.
        console.warn("Impossible d'extraire la tête du skin :", e);
        resolve(null);
      }
    };
    img.onerror = () => {
      clearTimeout(timeout);
      resolve(null);
    };
    img.src = skinUrl;
  });
}

// Met a jour l'affichage de l'avatar avec le skin si disponible
async function updateAvatarDisplay() {
  if (!discordUserCourant || !discordUserCourant.id) return;

  try {
    const hasCustomSkin = await invoke("has_custom_skin", {
      discordId: discordUserCourant.id,
    });

    if (hasCustomSkin) {
      // Extraire la tête du skin
      const skinUrl = `https://ouepamal.fr/skin-api/textures/${discordUserCourant.id}_skin.png`;
      const headDataUrl = await extractHeadFromSkin(skinUrl);

      // On ne bascule l'affichage vers l'image qu'une fois qu'on sait
      // qu'on a quelque chose a montrer, pour ne jamais laisser l'<img>
      // visible avec un src vide (icone cassee) pendant l'extraction.
      avatarSkin.onerror = () => {
        // Meme le skin complet ne charge pas : on retombe sur les initiales.
        avatarInitiale.classList.remove("cache");
        avatarSkin.classList.add("cache");
      };

      if (headDataUrl) {
        avatarSkin.src = headDataUrl;
      } else {
        avatarSkin.src = skinUrl; // Fallback au skin complet
      }

      avatarInitiale.classList.add("cache");
      avatarSkin.classList.remove("cache");
    } else {
      avatarInitiale.classList.remove("cache");
      avatarSkin.classList.add("cache");
      avatarInitiale.textContent = (usernameCourant || discordUserCourant.username || "?")[0].toUpperCase();
    }
  } catch (e) {
    console.error("Erreur mise a jour avatar:", e);
    avatarInitiale.classList.remove("cache");
    avatarSkin.classList.add("cache");
  }
}

// Charge le modèle de skin de l'utilisateur depuis la DB
async function loadSkinModel() {
  if (!discordUserCourant || !discordUserCourant.id) return;

  try {
    const model = await invoke("get_skin_model", {
      discordId: discordUserCourant.id,
    });

    // Par défaut, le modèle est "default" si pas encore dans la DB
    const actualModel = model || "default";
    skinModel = actualModel;

    if (actualModel === "slim") {
      modelAlex.checked = true;
      modelSteve.checked = false;
    } else {
      modelSteve.checked = true;
      modelAlex.checked = false;
    }
  } catch (e) {
    console.error("Erreur chargement modèle:", e);
    // En cas d'erreur, on met par défaut Steve
    skinModel = "default";
    modelSteve.checked = true;
    modelAlex.checked = false;
  }
}

// Met à jour le modèle dans la DB
async function updateSkinModelInDB() {
  if (!discordUserCourant || !discordUserCourant.id) return;

  try {
    await invoke("update_skin_model", {
      discordId: discordUserCourant.id,
      model: skinModel,
    });
    console.log("Modèle mis à jour:", skinModel);
  } catch (e) {
    console.error("Erreur mise à jour modèle:", e);
  }
}

// Charge la previsualisation du skin dans la modal
async function loadSkinPreview() {
  if (!discordUserCourant || !discordUserCourant.id) return;

  const skinUrl = `https://ouepamal.fr/skin-api/textures/${discordUserCourant.id}_skin.png`;
  const defaultSkinUrl = `https://ouepamal.fr/skin-api/textures/default_skin.png`;

  try {
    const hasCustomSkin = await invoke("has_custom_skin", {
      discordId: discordUserCourant.id,
    });

    const urlToLoad = hasCustomSkin ? skinUrl : defaultSkinUrl;
    skinPreviewImg.src = urlToLoad;

    if (hasCustomSkin) {
      skinStatus.textContent = "Skin custom actif";
      skinStatus.className = "skin-status success";
    } else {
      skinStatus.textContent = "Aucun skin custom. Le skin par défaut sera utilisé.";
      skinStatus.className = "skin-status";
    }

  } catch (e) {
    skinStatus.textContent = "Erreur de chargement: " + e;
    skinStatus.className = "skin-status error";
    console.error("Erreur:", e);
  }
}

// Ouvrir la modal de gestion du skin
async function openSkinModal() {
  if (!discordUserCourant || !discordUserCourant.id) return;

  await loadSkinModel();
  loadSkinPreview();
  modalSkin.classList.remove("cache");
}

// Fermer la modal
function closeSkinModal() {
  modalSkin.classList.add("cache");
  skinStatus.textContent = "";
  skinStatus.className = "skin-status";
}

// Valide les dimensions du skin (TOUS les skins Minecraft font 64x64 pixels)
function validateSkinDimensions(file) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const objectUrl = URL.createObjectURL(file);

    img.onload = () => {
      URL.revokeObjectURL(objectUrl);

      // Vérifier le format PNG
      if (!file.name.toLowerCase().endsWith('.png')) {
        reject("Le fichier doit être au format PNG");
        return;
      }

      // Tous les skins Minecraft font 64x64 pixels (slim vs default est dans le modèle, pas dans la taille)
      const expectedWidth = 64;
      const expectedHeight = 64;

      if (img.width !== expectedWidth || img.height !== expectedHeight) {
        reject(`Les dimensions du skin doivent être ${expectedWidth}x${expectedHeight} pixels. Votre image est ${img.width}x${img.height} pixels.`);
        return;
      }

      resolve();
    };

    img.onerror = () => {
      URL.revokeObjectURL(objectUrl);
      reject("Impossible de lire l'image. Vérifiez que c'est un fichier PNG valide.");
    };

    img.src = objectUrl;
  });
}

// Upload un nouveau skin
async function uploadNewSkin(file) {
  if (!discordUserCourant || !discordUserCourant.id || !file) return;

  if (file.size > 10 * 1024 * 1024) {
    skinStatus.textContent = "Le fichier est trop volumineux (max 10 Mo)";
    skinStatus.className = "skin-status error";
    return;
  }

  skinStatus.textContent = "Vérification du skin...";
  skinStatus.className = "skin-status";

  try {
    // Valider les dimensions du skin
    await validateSkinDimensions(file);

    skinStatus.textContent = "Upload en cours...";

    const arrayBuffer = await file.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);

    await invoke("upload_skin", {
      discordId: discordUserCourant.id,
      skinBytes: Array.from(bytes),
    });

    // Mettre à jour le modèle dans la DB
    await updateSkinModelInDB();

    skinStatus.textContent = "Skin uploadé avec succès !";
    skinStatus.className = "skin-status success";

    setTimeout(() => {
      updateAvatarDisplay();
      loadSkinPreview();
    }, 500);
  } catch (e) {
    skinStatus.textContent = e;
    skinStatus.className = "skin-status error";
  }
}

// Supprimer le skin custom
async function deleteCustomSkin() {
  if (!discordUserCourant || !discordUserCourant.id) return;

  if (!confirm("Etes-vous sur de vouloir supprimer votre skin custom ? Le skin par defaut sera utilise.")) {
    return;
  }

  skinStatus.textContent = "Suppression en cours...";
  skinStatus.className = "skin-status";

  try {
    await invoke("delete_skin", {
      discordId: discordUserCourant.id,
    });

    skinStatus.textContent = "Skin supprimé avec succès !";
    skinStatus.className = "skin-status success";

    setTimeout(() => {
      updateAvatarDisplay();
      loadSkinPreview();
    }, 500);
  } catch (e) {
    skinStatus.textContent = `Erreur : ${e}`;
    skinStatus.className = "skin-status error";
  }
}

// ============================================================================
// Gestion des News / Notifications
// ============================================================================

// Petite fonction d'échappement HTML : les news viennent du site admin, pas
// besoin qu'un titre/contenu mal formé casse le DOM (ou pire, injecte du HTML).
function echapperHtml(texte) {
  const div = document.createElement("div");
  div.textContent = texte ?? "";
  return div.innerHTML;
}

function formaterDateNews(iso) {
  if (!iso) return "";
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "";
    return d.toLocaleDateString("fr-FR", { day: "2-digit", month: "long", year: "numeric" });
  } catch {
    return "";
  }
}

// Affiche une liste de news dans un conteneur donné.
// `avecContenu` : true pour la modal complète (bouton "Lire la suite"),
// false pour le mini-dropdown de notifications (juste titre + date).
function rendreListeNews(conteneur, items, avecContenu) {
  conteneur.innerHTML = "";

  if (!items.length) {
    conteneur.innerHTML = `<p class="news-vide">Aucune news pour le moment.</p>`;
    return;
  }

  for (const item of items) {
    const el = document.createElement("article");
    el.className = "news-item";

    el.innerHTML = `
      ${item.cover_url ? `<img class="news-item-cover" src="${item.cover_url}" alt="" />` : ""}
      <div class="news-item-corps">
        <p class="news-item-date">${formaterDateNews(item.created_at)}</p>
        <h3 class="news-item-titre">${echapperHtml(item.title)}</h3>
        <p class="news-item-extrait">${echapperHtml(item.excerpt)}</p>
        ${avecContenu && item.content ? `
          <p class="news-item-contenu cache">${echapperHtml(item.content)}</p>
          <button type="button" class="news-item-toggle">Lire la suite</button>
        ` : ""}
      </div>
    `;

    if (avecContenu && item.content) {
      const btnToggle = el.querySelector(".news-item-toggle");
      const contenuEl = el.querySelector(".news-item-contenu");
      btnToggle.addEventListener("click", () => {
        const estCache = contenuEl.classList.toggle("cache");
        btnToggle.textContent = estCache ? "Lire la suite" : "Réduire";
      });
    }

    conteneur.appendChild(el);
  }
}

// Marque toutes les news actuellement chargées comme "vues" (retire le
// point rouge de notification jusqu'à la prochaine news publiée).
function marquerNewsCommeVues() {
  if (!newsCourantes.length) return;
  localStorage.setItem(CLE_DERNIERE_NEWS_VUE, newsCourantes[0].id);
  notifDot.classList.add("cache");
}

// Met à jour l'aperçu sur l'accueil + le point de notification + le
// mini-dropdown, à partir de `newsCourantes` déjà chargées.
function mettreAJourApercuEtNotifs() {
  if (!newsCourantes.length) {
    apercuNews.classList.add("cache");
    notifDot.classList.add("cache");
    rendreListeNews(notifListeEl, [], false);
    return;
  }

  const derniere = newsCourantes[0];

  apercuNewsTitre.textContent = derniere.title;
  if (apercuNewsExtrait) {
    apercuNewsExtrait.textContent = derniere.excerpt || '';
  }
  if (derniere.cover_url) {
    apercuNewsCover.src = derniere.cover_url;
    apercuNewsCover.classList.remove("cache");
  } else {
    apercuNewsCover.classList.add("cache");
  }
  apercuNews.classList.remove("cache");

  const derniereVue = localStorage.getItem(CLE_DERNIERE_NEWS_VUE);
  if (derniereVue !== derniere.id) {
    notifDot.classList.remove("cache");
  } else {
    notifDot.classList.add("cache");
  }

  rendreListeNews(notifListeEl, newsCourantes.slice(0, 5), false);
}

// Récupère les news depuis le site admin via la commande Tauri fetch_news
// (elle-même vers index.php?api=news, voir news.rs côté Rust).
async function chargerNews() {
  try {
    const news = await invoke("fetch_news");
    newsCourantes = Array.isArray(news) ? news : [];
    mettreAJourApercuEtNotifs();
  } catch (e) {
    console.warn("Impossible de charger les news :", e);
  }
}

function fermerModalNews() {
  modalNews.classList.add("cache");
}

function ouvrirModalNews() {
  rendreListeNews(newsListeEl, newsCourantes, true);
  modalNews.classList.remove("cache");
  notifDropdown.classList.add("cache");
  marquerNewsCommeVues();
}

// ============================================================================
// Gestion des onglets (via sidebar)
// ============================================================================

function activerOnglet(ongletAActiver, btnAActiver) {
  // Désactiver tous les onglets et boutons
  [ongletAccueil, ongletNews].forEach(onglet => onglet && onglet.classList.remove("onglet--actif"));
  [btnNavAccueil, btnNavNews].forEach(btn => btn && btn.classList.remove("nav-icone--actif"));

  // Activer l'onglet et le bouton demandés
  ongletAActiver.classList.add("onglet--actif");
  btnAActiver.classList.add("nav-icone--actif");

  // Charger le contenu de l'onglet si nécessaire
  if (ongletAActiver === ongletNews && newsCourantes.length > 0) {
    rendreNewsDansOnglet();
  }
}

function renderNewsCard(item) {
  const card = document.createElement("article");
  card.className = "news-card";

  card.innerHTML = `
    <div class="news-card-image">
      ${item.cover_url ? `<img class="news-card-cover" src="${item.cover_url}" alt="" />` : ''}
    </div>
    <div class="news-card-texte">
      <span class="news-card-date">${formaterDateNews(item.created_at)}</span>
      <h3 class="news-card-titre">${echapperHtml(item.title)}</h3>
      <p class="news-card-extrait">${echapperHtml(item.excerpt || '')}</p>
    </div>
  `;

  card.addEventListener('click', () => {
    ouvrirModalNewsAvecDetail(item);
  });

  return card;
}

function rendreNewsDansOnglet() {
  newsListeOngletEl.innerHTML = "";

  if (!newsCourantes.length) {
    newsListeOngletEl.innerHTML = `<p class="news-vide">Aucune news pour le moment.</p>`;
    return;
  }

  for (const item of newsCourantes) {
    const card = renderNewsCard(item);
    newsListeOngletEl.appendChild(card);
  }
}

function ouvrirModalNewsAvecDetail(item) {
  newsListeEl.innerHTML = "";
  
  const detailEl = document.createElement("article");
  detailEl.className = "news-item";
  
  detailEl.innerHTML = `
    ${item.cover_url ? `<img class="news-item-cover" src="${item.cover_url}" alt="" style="width: 120px; height: 120px; border-radius: 12px;" />` : ""}
    <div class="news-item-corps">
      <p class="news-item-date">${formaterDateNews(item.created_at)}</p>
      <h3 class="news-item-titre">${echapperHtml(item.title)}</h3>
      <p class="news-item-extrait">${echapperHtml(item.excerpt)}</p>
      ${item.content ? `
        <p class="news-item-contenu">${echapperHtml(item.content)}</p>
      ` : ""}
    </div>
  `;
  
  newsListeEl.appendChild(detailEl);
  modalNews.classList.remove("cache");
  notifDropdown.classList.add("cache");
  marquerNewsCommeVues();
}

// La fenetre demarre au format vertical (ecran de connexion).
definirTailleFenetre(TAILLE_VERTICALE.largeur, TAILLE_VERTICALE.hauteur);

btnDiscord.addEventListener("click", async () => {
  erreur.classList.add("cache");
  statutAuth.textContent = "Ouverture du navigateur...";

  try {
    const authUrl = await invoke("start_discord_auth");
    await openUrl(authUrl);

    statutAuth.textContent = "En attente de la connexion dans le navigateur...";
    const discordUser = await invoke("complete_discord_auth");
    discordUserCourant = discordUser;

    const existingUser = await invoke("get_user_by_discord_id", {
      discordId: discordUser.id,
    });

    if (existingUser) {
      usernameCourant = existingUser.username;
      await definirProfil(existingUser.username);
      afficherEcran(ecranConnecte);
    } else {
      document.getElementById("bienvenue-discord").textContent =
          `Bienvenue, ${discordUser.username} ! Choisis ton pseudo Minecraft :`;
      afficherEcran(ecranPseudo);
    }
  } catch (e) {
    afficherErreur(`Erreur : ${e}`);
    statutAuth.textContent = "";
  }
});

btnValiderPseudo.addEventListener("click", async () => {
  const pseudo = document.getElementById("input-pseudo").value.trim();
  if (!pseudo) return;

  try {
    const user = await invoke("create_user", {
      discordId: discordUserCourant.id,
      username: pseudo,
    });
    usernameCourant = user.username;
    await definirProfil(user.username);
    afficherEcran(ecranConnecte);
  } catch (e) {
    afficherErreur(`Erreur : ${e}`);
  }
});

btnJouer.addEventListener("click", async () => {
  if (!usernameCourant) return;

  erreur.classList.add("cache");
  btnJouer.disabled = true;
  const contenuOriginal = btnJouer.innerHTML;
  btnJouer.textContent = "Installation / Lancement...";

  try {
    await invoke("launch_game", { username: usernameCourant });
  } catch (e) {
    afficherErreur(`Erreur : ${e}`);
  } finally {
    btnJouer.disabled = false;
    btnJouer.innerHTML = contenuOriginal;
  }
});

// Ecouteurs d'evenements pour la gestion du skin
if (btnProfil) btnProfil.addEventListener("click", openSkinModal);
if (btnCloseModal) btnCloseModal.addEventListener("click", closeSkinModal);
if (btnDeleteSkin) btnDeleteSkin.addEventListener("click", deleteCustomSkin);

// Gestion du changement de modèle
if (modelSteve && modelAlex) {
  modelSteve.addEventListener("change", async () => {
    if (modelSteve.checked) {
      skinModel = "default";
      await updateSkinModelInDB();
    }
  });

  modelAlex.addEventListener("change", async () => {
    if (modelAlex.checked) {
      skinModel = "slim";
      await updateSkinModelInDB();
    }
  });
}

if (inputSkinUpload) {
  inputSkinUpload.addEventListener("change", (e) => {
    if (e.target.files && e.target.files.length > 0) {
      uploadNewSkin(e.target.files[0]);
      e.target.value = "";
    }
  });
}

if (modalSkin) {
  modalSkin.addEventListener("click", (e) => {
    if (e.target === modalSkin) {
      closeSkinModal();
    }
  });
}

// Ecouteurs d'evenements pour les news / notifications
if (btnNavNews) btnNavNews.addEventListener("click", () => activerOnglet(ongletNews, btnNavNews));
if (btnNavAccueil) btnNavAccueil.addEventListener("click", () => activerOnglet(ongletAccueil, btnNavAccueil));
if (apercuNews) {
  apercuNews.addEventListener("click", (e) => {
    e.preventDefault();
    activerOnglet(ongletNews, btnNavNews);
  });
}
if (btnCloseNewsModal) btnCloseNewsModal.addEventListener("click", fermerModalNews);
if (modalNews) {
  modalNews.addEventListener("click", (e) => {
    if (e.target === modalNews) {
      fermerModalNews();
    }
  });
}

if (btnNotifications) {
  btnNotifications.addEventListener("click", (e) => {
    e.stopPropagation();
    notifDropdown.classList.toggle("cache");
  });
}

// Ferme le dropdown de notifications si on clique n'importe où ailleurs.
document.addEventListener("click", (e) => {
  if (!notifDropdown || notifDropdown.classList.contains("cache")) return;
  if (notifDropdown.contains(e.target)) return;
  if (btnNotifications && btnNotifications.contains(e.target)) return;
  notifDropdown.classList.add("cache");
});

// Mettre a jour le profil (avatar + tooltip) quand on se connecte.
// Le nom du joueur n'est plus affiché en texte visible dans la sidebar
// (la réf n'affiche que l'avatar) : il reste disponible au survol de
// l'avatar (title) et pour les lecteurs d'écran (#texte-connecte, sr-only).
function definirProfil(nom) {
  document.getElementById("texte-connecte").textContent = `Connecte en tant que ${nom}`;
  avatarInitiale.textContent = (nom[0] || "?").toUpperCase();
  if (btnProfil) {
    btnProfil.title = `${nom} — gérer mon skin`;
  }

  // Charger l'avatar et le modèle si on a discordUserCourant
  if (discordUserCourant && discordUserCourant.id) {
    updateAvatarDisplay();
    loadSkinModel();
  }

  // Charger les news (aperçu accueil + notifications), indépendamment du
  // profil Discord — c'est juste au moment où on arrive sur l'écran de jeu.
  chargerNews();
}