const { invoke } = window.__TAURI__.core;
const { openUrl } = window.__TAURI__.opener;
const tauriWindow = window.__TAURI__.window;
const tauriEvent = window.__TAURI__.event;

const ecranConnexion = document.getElementById("ecran-connexion");
const ecranPseudo = document.getElementById("ecran-pseudo");
const ecranConnecte = document.getElementById("ecran-connecte");
const statutAuth = document.getElementById("statut-auth");
const erreur = document.getElementById("erreur");
const avatarInitiale = document.getElementById("avatar-initiale");

const btnDiscord = document.getElementById("btn-discord");
const btnValiderPseudo = document.getElementById("btn-valider-pseudo");
const btnJouer = document.getElementById("btn-jouer");
const launchProgress = document.getElementById("launch-progress");
const launchProgressLabel = document.getElementById("launch-progress-label");
const launchProgressPercent = document.getElementById("launch-progress-percent");
const launchProgressTrack = document.getElementById("launch-progress-track");
const launchProgressFill = document.getElementById("launch-progress-fill");
const launchProgressDetail = document.getElementById("launch-progress-detail");

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
const btnGotoCapes = document.getElementById("btn-goto-capes");

// Gestion des news / notifications
const btnNavNews = document.getElementById("btn-nav-news");
const btnNavAccueil = document.getElementById("btn-nav-accueil");
const btnNavCapes = document.getElementById("btn-nav-capes");
const newsListeEl = document.getElementById("news-liste");
const apercuNews = document.getElementById("apercu-news");
const apercuNewsCover = document.getElementById("apercu-news-cover");
const apercuNewsTitre = document.getElementById("apercu-news-titre");
const apercuNewsExtrait = document.getElementById("apercu-news-extrait");
const apercuNewsCategory = document.getElementById("apercu-news-category");
const homeNewsList = document.getElementById("home-news-list");
const btnVoirNews = document.getElementById("btn-voir-news");

// Éléments pour les onglets
const ongletAccueil = document.getElementById("onglet-accueil");
const ongletNews = document.getElementById("onglet-news");
const ongletCapes = document.getElementById("onglet-capes");
const newsListeOngletEl = document.getElementById("news-liste-onglet");
const newsFeaturedEl = document.getElementById("news-featured");
const newsDetailEl = document.getElementById("news-detail");
const btnNotifications = document.getElementById("btn-notifications");
const notifDropdown = document.getElementById("notif-dropdown");
const notifListeEl = document.getElementById("notif-liste");
const notifDot = document.getElementById("notif-dot");

// Boutique et collection de capes
const capeWalletBalance = document.getElementById("cape-wallet-balance");
const capeShopFeedback = document.getElementById("cape-shop-feedback");
const capeShopSummary = document.getElementById("cape-shop-summary");
const capeShopGrid = document.getElementById("cape-shop-grid");
const btnRemoveCape = document.getElementById("btn-remove-cape");

// Éléments pour la page detail news
const newsDetailCoverContainer = document.getElementById("news-detail-cover-container");
const newsDetailCover = document.getElementById("news-detail-cover");
const newsDetailDate = document.getElementById("news-detail-date");
const newsDetailTitle = document.getElementById("news-detail-title");
const newsDetailContent = document.getElementById("news-detail-content");
const btnBackToNews = document.getElementById("btn-back-to-news");

let discordUserCourant = null;
let usernameCourant = null;
let skinModel = "default";
let newsCourantes = [];
let lancementEnCours = false;
let jeuLance = false;
let cosmeticsProfile = null;

// Ecrans de connexion/pseudo : format vertical.
// Ecran de jeu : format fenetre classique, comme un launcher normal.
const TAILLE_VERTICALE = { largeur: 480, hauteur: 760 };
const TAILLE_JEU = { largeur: 1280, hauteur: 760 };

// Clé locale (persistée entre lancements) qui retient l'id de la dernière
// news déjà vue par ce joueur, pour savoir s'il faut afficher le point
// rouge de notification.
const CLE_DERNIERE_NEWS_VUE = "idc_derniere_news_vue";

async function definirTailleFenetre(largeur, hauteur) {
  const estHub = largeur === TAILLE_JEU.largeur;

  // La commande native est la source de vérité : macOS/WebKit peut ignorer
  // des appels `setSize` envoyés depuis le WebView après une navigation.
  try {
    await invoke("set_launcher_window_mode", { mode: estHub ? "hub" : "login" });
    return;
  } catch (e) {
    console.warn("Redimensionnement natif indisponible, fallback WebView :", e);
  }

  if (!tauriWindow) return;
  const fenetre = tauriWindow.getCurrentWindow();
  const taille = new tauriWindow.LogicalSize(largeur, hauteur);
  const tailleMinimum = new tauriWindow.LogicalSize(estHub ? 1120 : 480, estHub ? 680 : 720);

  try {
    await fenetre.setMinSize(
        tailleMinimum
    );
  } catch (e) {
    // Un minimum de fenêtre non supporté ne doit jamais empêcher le hub
    // de reprendre sa largeur normale.
    console.warn("Impossible de définir la taille minimale :", e);
  }

  try {
    await fenetre.setSize(taille);
    await fenetre.center();
  } catch (e) {
    console.warn("Impossible de redimensionner la fenêtre :", e);
  }
}

async function afficherEcran(ecran) {
  if (ecran === ecranConnecte) {
    await definirTailleFenetre(TAILLE_JEU.largeur, TAILLE_JEU.hauteur);
  } else {
    await definirTailleFenetre(TAILLE_VERTICALE.largeur, TAILLE_VERTICALE.hauteur);
  }

  [ecranConnexion, ecranPseudo, ecranConnecte].forEach((e) => e.classList.add("cache"));
  ecran.classList.remove("cache");
}

function afficherErreur(message) {
  erreur.textContent = message;
  erreur.classList.remove("cache");
}

function mettreAJourProgressionLancement(update) {
  if (!launchProgress) return;

  const progress = Math.max(0, Math.min(100, Number(update?.progress) || 0));
  const phase = update?.phase || "preparing";
  launchProgress.classList.remove("cache");
  launchProgress.dataset.phase = phase;
  launchProgress.classList.toggle("launch-progress--termine", phase === "started");
  launchProgress.classList.toggle("launch-progress--erreur", phase === "error");

  if (launchProgressLabel) {
    launchProgressLabel.textContent = update?.label || "Préparation du launcher";
  }
  if (launchProgressPercent) launchProgressPercent.textContent = `${progress}%`;
  if (launchProgressDetail) {
    launchProgressDetail.textContent = update?.detail || "Cette étape peut prendre quelques instants.";
  }
  if (launchProgressFill) launchProgressFill.style.width = `${progress}%`;
  if (launchProgressTrack) launchProgressTrack.setAttribute("aria-valuenow", String(progress));
}

// Le backend envoie uniquement de vraies étapes du lancement : la barre ne
// prétend donc jamais connaître un nombre d'octets qu'il ne peut pas mesurer.
if (tauriEvent?.listen) {
  tauriEvent.listen("launcher-progress", ({ payload }) => {
    mettreAJourProgressionLancement(payload);
  }).catch((error) => {
    console.warn("Écoute de la progression indisponible :", error);
  });
}

// ============================================================================
// Gestion du Skin
// ============================================================================

// Extrait la tête et sa seconde couche (casque/chapeau) d'un skin Minecraft.
// Le rendu garde le pixel-art net tout en affichant la tête complète.
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
        // La surcouche de tête (chapeau, casque, cheveux...) est à (40,8).
        // Elle donne le relief caractéristique des avatars de launcher.
        if (img.width >= 48 && img.height >= 16) {
          ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 40, 40);
        }

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

    // L'identité visuelle ne dépend jamais du pseudo Minecraft : celui-ci est
    // libre dans le launcher. On part exclusivement du skin associé à l'ID
    // Discord par l'API IDC, puis on en extrait la tête localement.
    const skinUrl = hasCustomSkin
        ? `https://ouepamal.fr/skin-api/textures/${discordUserCourant.id}_skin.png`
        : "https://ouepamal.fr/skin-api/textures/default_skin.png";
    const headDataUrl = await extractHeadFromSkin(skinUrl);

    avatarSkin.onerror = () => {
      avatarInitiale.textContent = "?";
      avatarInitiale.classList.remove("cache");
      avatarSkin.classList.add("cache");
    };

    if (headDataUrl) {
      avatarSkin.src = headDataUrl;
      avatarInitiale.classList.add("cache");
      avatarSkin.classList.remove("cache");
    } else {
      // L'API doit renvoyer des PNG avec CORS pour l'extraction canvas. En
      // cas d'indisponibilité, on préfère une initiale neutre à un pseudo
      // pouvant représenter un autre joueur ou un mauvais skin.
      avatarInitiale.textContent = "?";
      avatarInitiale.classList.remove("cache");
      avatarSkin.classList.add("cache");
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
// Capes / portefeuille
// ============================================================================

function afficherRetourCape(message, type = "info") {
  if (!capeShopFeedback) return;
  capeShopFeedback.textContent = message;
  capeShopFeedback.className = `cape-shop-feedback cape-shop-feedback--${type}`;
  capeShopFeedback.classList.remove("cache");
}

function formatNombreEclats(value) {
  return new Intl.NumberFormat("fr-FR").format(Number(value) || 0);
}

function rendreBoutiqueCapes(profile) {
  if (!profile || !capeShopGrid) return;
  cosmeticsProfile = profile;
  const capes = Array.isArray(profile.capes) ? profile.capes : [];
  const selectedCapeId = profile.selectedCapeId || null;

  if (capeWalletBalance) capeWalletBalance.textContent = formatNombreEclats(profile.balance);
  if (capeShopSummary) {
    const possedees = capes.filter((cape) => cape.owned).length;
    capeShopSummary.textContent = possedees
        ? `${possedees} cape${possedees > 1 ? "s" : ""} dans ta collection`
        : "Ta collection attend sa première cape.";
  }
  if (btnRemoveCape) btnRemoveCape.classList.toggle("cache", !selectedCapeId);

  capeShopGrid.innerHTML = "";
  if (!capes.length) {
    capeShopGrid.innerHTML = `<p class="cape-shop-empty">Aucune cape n'est disponible pour le moment.</p>`;
    return;
  }

  for (const cape of capes) {
    const card = document.createElement("article");
    card.className = `cape-card${cape.selected ? " cape-card--selected" : ""}${cape.owned ? " cape-card--owned" : ""}`;
    const price = formatNombreEclats(cape.price);
    const action = cape.owned
        ? (cape.selected
            ? `<span class="cape-card-active">Équipée</span>`
            : `<button class="cape-card-action cape-card-action--select" type="button" data-cape-action="select" data-cape-id="${echapperHtml(cape.id)}">Équiper</button>`)
        : `<button class="cape-card-action" type="button" data-cape-action="buy" data-cape-id="${echapperHtml(cape.id)}">Débloquer <span>${price} ✦</span></button>`;

    card.innerHTML = `
      <div class="cape-card-art">
        <span class="cape-card-glow" aria-hidden="true"></span>
        <img src="${echapperHtml(cape.textureUrl)}" alt="Aperçu de la cape ${echapperHtml(cape.name)}" />
        ${cape.selected ? '<span class="cape-card-badge">Active</span>' : ""}
        ${cape.owned && !cape.selected ? '<span class="cape-card-badge cape-card-badge--owned">Possédée</span>' : ""}
      </div>
      <div class="cape-card-copy">
        <h3>${echapperHtml(cape.name)}</h3>
        <p>${echapperHtml(cape.description || "Une pièce rare de la collection de l'île.")}</p>
        <div class="cape-card-footer">
          ${cape.owned ? '<span class="cape-card-price">Dans ta collection</span>' : `<span class="cape-card-price">${price} éclats</span>`}
          ${action}
        </div>
      </div>
    `;
    capeShopGrid.appendChild(card);
  }
}

async function chargerBoutiqueCapes() {
  if (!discordUserCourant?.id) return;
  if (capeShopSummary) capeShopSummary.textContent = "Synchronisation du portefeuille…";
  try {
    const profile = await invoke("get_cape_shop", { discordId: discordUserCourant.id });
    rendreBoutiqueCapes(profile);
    if (capeShopFeedback) capeShopFeedback.classList.add("cache");
  } catch (error) {
    afficherRetourCape(`Impossible de charger les capes : ${error}`, "error");
    if (capeShopSummary) capeShopSummary.textContent = "Collection indisponible pour le moment.";
  }
}

async function acheterCape(capeId) {
  if (!discordUserCourant?.id) return;
  afficherRetourCape("Achat en cours…");
  try {
    const profile = await invoke("purchase_cape", { discordId: discordUserCourant.id, capeId });
    rendreBoutiqueCapes(profile);
    afficherRetourCape("Cape débloquée et ajoutée à ta collection !", "success");
  } catch (error) {
    afficherRetourCape(String(error), "error");
  }
}

async function selectionnerCape(capeId) {
  if (!discordUserCourant?.id) return;
  afficherRetourCape(capeId ? "Équipement de la cape…" : "Retrait de la cape…");
  try {
    const profile = await invoke("select_cape", { discordId: discordUserCourant.id, capeId: capeId || null });
    rendreBoutiqueCapes(profile);
    afficherRetourCape(capeId ? "Cape équipée. Elle sera visible en jeu après la mise à jour du mod." : "Aucune cape n'est maintenant équipée.", "success");
  } catch (error) {
    afficherRetourCape(String(error), "error");
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

function categorieNews(item) {
  return item.category || "Actualité";
}

function texteApercu(texte) {
  return (texte || "")
      .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
      .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
      .replace(/[*_`#>]/g, "")
      .replace(/\s+/g, " ")
      .trim();
}

function resoudreUrlNews(url, item) {
  if (!url || url.startsWith("#")) return null;

  const base = item?.content_base_url || item?.cover_url || null;
  if (!base) return null;

  try {
    const ressource = new URL(url, base);
    return ["https:", "http:", "mailto:", "tel:"].includes(ressource.protocol)
        ? ressource.href
        : null;
  } catch {
    return null;
  }
}

function preparerRessourcesMarkdown(conteneur, item) {
  conteneur.querySelectorAll("img[src]").forEach((image) => {
    const src = resoudreUrlNews(image.getAttribute("src"), item);
    if (!src) {
      image.remove();
      return;
    }

    image.src = src;
    image.loading = "lazy";
    image.decoding = "async";
    image.addEventListener("error", () => image.classList.add("markdown-image--erreur"), { once: true });
  });

  conteneur.querySelectorAll("a[href]").forEach((lien) => {
    const href = resoudreUrlNews(lien.getAttribute("href"), item);
    if (!href) {
      lien.removeAttribute("href");
      return;
    }

    lien.href = href;
    lien.target = "_blank";
    lien.rel = "noopener noreferrer";
  });
}

function rendreMarkdown(texte) {
  const source = String(texte || "");
  if (typeof marked === "undefined") {
    return `<p>${echapperHtml(source)}</p>`;
  }

  const html = marked.parse(source, { breaks: true, gfm: true });
  if (typeof DOMPurify !== "undefined") {
    return DOMPurify.sanitize(html, {
      USE_PROFILES: { html: true },
      ADD_TAGS: ["img"],
      ADD_ATTR: ["src", "alt", "title", "width", "height"],
      FORBID_TAGS: ["style", "script", "iframe", "object", "embed", "form", "input", "button"],
    });
  }

  // Le contenu est administré, mais on ne rend jamais du HTML brut lorsqu'un
  // CDN est temporairement indisponible.
  return `<p>${echapperHtml(source)}</p>`;
}

function newsALaUne() {
  return newsCourantes.find((item) => item.featured) || newsCourantes[0];
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

    const excerpt = echapperHtml(texteApercu(item.excerpt));
    const contentHtml = avecContenu && item.content ? rendreMarkdown(item.content) : "";

    el.innerHTML = `
      ${item.cover_url ? `<img class="news-item-cover" src="${item.cover_url}" alt="" />` : ""}
      <div class="news-item-corps">
        <p class="news-item-date">${formaterDateNews(item.created_at)}</p>
        <h3 class="news-item-titre">${echapperHtml(item.title)}</h3>
        <p class="news-item-extrait">${excerpt}</p>
        ${avecContenu && item.content ? `
          <div class="news-item-contenu cache">${contentHtml}</div>
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
    if (homeNewsList) homeNewsList.innerHTML = "";
    return;
  }

  const derniere = newsCourantes[0];
  const une = newsALaUne();

  apercuNewsTitre.textContent = une.title;
  apercuNewsExtrait.textContent = texteApercu(une.excerpt);
  if (apercuNewsCategory) apercuNewsCategory.textContent = categorieNews(une);
  if (une.cover_url) {
    apercuNewsCover.src = une.cover_url;
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
  rendreNewsAccueil(une);
}

function rendreNewsAccueil(articleUne) {
  if (!homeNewsList) return;
  homeNewsList.innerHTML = "";

  newsCourantes
      .filter((item) => item.id !== articleUne.id)
      .slice(0, 2)
      .forEach((item) => {
        const article = document.createElement("button");
        article.type = "button";
        article.className = "home-news-item";
        article.innerHTML = `
        <span class="home-news-item-title">${echapperHtml(item.title)}</span>
        <span class="home-news-item-date">${formaterDateNews(item.created_at)}</span>
      `;
        article.addEventListener("click", () => afficherNewsDetail(item));
        homeNewsList.appendChild(article);
      });
}

// Récupère les news depuis le site admin via la commande Tauri fetch_news
// (elle-même vers index.php?api=news, voir news.rs côté Rust).
async function chargerNews() {
  try {
    const news = await invoke("fetch_news");
    newsCourantes = Array.isArray(news) ? news : [];
    mettreAJourApercuEtNotifs();

    // Si on est déjà sur l'onglet news, mettre à jour la liste
    if (ongletNews && ongletNews.classList.contains("onglet--actif")) {
      rendreNewsDansOnglet();
    }
  } catch (e) {
    console.warn("Impossible de charger les news :", e);
  }
}



// ============================================================================
// Gestion des onglets (via sidebar)
// ============================================================================

function activerOnglet(ongletAActiver, btnAActiver) {
  // Désactiver tous les onglets et boutons
  [ongletAccueil, ongletNews, ongletCapes, newsDetailEl].forEach(onglet => onglet && onglet.classList.remove("onglet--actif"));
  [btnNavAccueil, btnNavNews, btnNavCapes].forEach(btn => btn && btn.classList.remove("nav-icone--actif"));

  // Activer l'onglet et le bouton demandés
  ongletAActiver.classList.add("onglet--actif");
  btnAActiver.classList.add("nav-icone--actif");

  // Charger le contenu de l'onglet si nécessaire
  if (ongletAActiver === ongletNews && newsCourantes.length > 0) {
    rendreNewsDansOnglet();
  }
  if (ongletAActiver === ongletCapes) {
    chargerBoutiqueCapes();
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
      <span class="news-card-date">${categorieNews(item)} · ${formaterDateNews(item.created_at)}</span>
      <h3 class="news-card-titre">${echapperHtml(item.title)}</h3>
      <p class="news-card-extrait">${echapperHtml(texteApercu(item.excerpt))}</p>
    </div>
  `;

  card.addEventListener('click', () => {
    afficherNewsDetail(item);
  });

  return card;
}

function renderNewsFeatured(item) {
  const article = document.createElement("article");
  article.className = "news-featured-card";
  article.innerHTML = `
    <div class="news-featured-image">
      ${item.cover_url ? `<img src="${item.cover_url}" alt="" />` : ""}
    </div>
    <div class="news-featured-copy">
      <span class="news-card-date">${categorieNews(item)}</span>
      <span class="news-featured-date">${formaterDateNews(item.created_at)}</span>
      <h3 class="news-featured-title">${echapperHtml(item.title)}</h3>
      <p class="news-featured-excerpt">${echapperHtml(texteApercu(item.excerpt))}</p>
      <span class="news-featured-action">Lire l'article <span aria-hidden="true">→</span></span>
    </div>
  `;
  article.addEventListener("click", () => afficherNewsDetail(item));
  return article;
}

// Affiche une news en plein écran (fullscreen)
function afficherNewsDetail(item) {
  if (!item || !newsDetailEl) return;

  // Désactiver tous les onglets
  [ongletAccueil, ongletNews].forEach(onglet => onglet && onglet.classList.remove("onglet--actif"));
  [btnNavAccueil, btnNavNews].forEach(btn => btn && btn.classList.remove("nav-icone--actif"));

  // Activer l'onglet detail
  newsDetailEl.classList.remove("cache");
  newsDetailEl.classList.add("onglet--actif");

  // Marquer les news comme vues
  marquerNewsCommeVues();

  // Remplir les données de la news
  if (item.cover_url) {
    newsDetailCover.src = item.cover_url;
    newsDetailCoverContainer.classList.remove("cache");
  } else {
    newsDetailCoverContainer.classList.add("cache");
  }

  newsDetailDate.textContent = formaterDateNews(item.created_at);
  newsDetailTitle.textContent = item.title;

  newsDetailContent.innerHTML = rendreMarkdown(item.content);
  preparerRessourcesMarkdown(newsDetailContent, item);
  const viewport = newsDetailEl.querySelector(".news-detail-contenu");
  if (viewport) viewport.scrollTop = 0;

  // Fermer le dropdown de notifications
  if (notifDropdown) {
    notifDropdown.classList.add("cache");
  }
}

function rendreNewsDansOnglet() {
  newsListeOngletEl.innerHTML = "";
  if (newsFeaturedEl) newsFeaturedEl.innerHTML = "";

  if (!newsCourantes.length) {
    newsListeOngletEl.innerHTML = `<p class="news-vide">Aucune news pour le moment.</p>`;
    return;
  }

  const une = newsALaUne();
  if (newsFeaturedEl) newsFeaturedEl.appendChild(renderNewsFeatured(une));

  const articles = newsCourantes.filter((item) => item.id !== une.id);
  for (const item of articles) {
    const card = renderNewsCard(item);
    newsListeOngletEl.appendChild(card);
  }

  if (!articles.length) {
    newsListeOngletEl.innerHTML = `<p class="news-vide">D'autres chroniques arriveront bientôt.</p>`;
  }
}

// Fonction pour revenir à la liste des news
function revenirAListeNews() {
  if (!newsDetailEl || !ongletNews) return;

  // Désactiver l'onglet detail
  newsDetailEl.classList.remove("onglet--actif");
  newsDetailEl.classList.add("cache");

  // Activer l'onglet news
  activerOnglet(ongletNews, btnNavNews);
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
      await afficherEcran(ecranConnecte);
    } else {
      document.getElementById("bienvenue-discord").textContent =
          `Bienvenue, ${discordUser.username} ! Choisis ton pseudo Minecraft :`;
      await afficherEcran(ecranPseudo);
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
    await afficherEcran(ecranConnecte);
  } catch (e) {
    afficherErreur(`Erreur : ${e}`);
  }
});

btnJouer.addEventListener("click", async () => {
  if (!usernameCourant || lancementEnCours || jeuLance) return;

  erreur.classList.add("cache");
  lancementEnCours = true;
  btnJouer.disabled = true;
  const contenuOriginal = btnJouer.innerHTML;
  btnJouer.textContent = "Préparation...";
  mettreAJourProgressionLancement({
    phase: "preparing",
    progress: 4,
    label: "Préparation du launcher",
    detail: "Vérification de ton installation",
  });

  try {
    await invoke("launch_game", { username: usernameCourant });
    mettreAJourProgressionLancement({
      phase: "started",
      progress: 100,
      label: "Minecraft est lancé",
      detail: "Bon jeu sur L'île des Cobayes !",
    });
    jeuLance = true;
  } catch (e) {
    afficherErreur(`Erreur : ${e}`);
    mettreAJourProgressionLancement({
      phase: "error",
      progress: 0,
      label: "Lancement interrompu",
      detail: "Une erreur a empêché le démarrage du jeu.",
    });
  } finally {
    lancementEnCours = false;
    if (jeuLance) {
      btnJouer.disabled = true;
      btnJouer.classList.add("btn-jouer--lance");
      btnJouer.innerHTML = `
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="m5 12 4.2 4.2L19.5 6" />
        </svg>
        Lancé
      `;
    } else {
      btnJouer.disabled = false;
      btnJouer.innerHTML = contenuOriginal;
    }
  }
});

// Ecouteurs d'evenements pour la gestion du skin
if (btnProfil) btnProfil.addEventListener("click", openSkinModal);
if (btnCloseModal) btnCloseModal.addEventListener("click", closeSkinModal);
if (btnDeleteSkin) btnDeleteSkin.addEventListener("click", deleteCustomSkin);
if (btnGotoCapes) {
  btnGotoCapes.addEventListener("click", () => {
    closeSkinModal();
    activerOnglet(ongletCapes, btnNavCapes);
  });
}

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
if (btnNavCapes) btnNavCapes.addEventListener("click", () => activerOnglet(ongletCapes, btnNavCapes));
if (btnVoirNews) btnVoirNews.addEventListener("click", () => activerOnglet(ongletNews, btnNavNews));
if (apercuNews) {
  apercuNews.addEventListener("click", (e) => {
    e.preventDefault();
    // Si on a des news chargées, afficher la première (la plus récente) en fullscreen
    if (newsCourantes && newsCourantes.length > 0) {
      afficherNewsDetail(newsCourantes[0]);
    } else {
      activerOnglet(ongletNews, btnNavNews);
    }
  });
}
if (btnBackToNews) btnBackToNews.addEventListener("click", revenirAListeNews);

// Ecouteurs d'evenements pour la boutique de capes
if (capeShopGrid) {
  capeShopGrid.addEventListener("click", (e) => {
    const bouton = e.target.closest("[data-cape-action]");
    if (!bouton) return;
    const capeId = bouton.dataset.capeId;
    const action = bouton.dataset.capeAction;
    if (!capeId) return;
    if (action === "buy") acheterCape(capeId);
    if (action === "select") selectionnerCape(capeId);
  });
}
if (btnRemoveCape) {
  btnRemoveCape.addEventListener("click", () => selectionnerCape(null));
}

// Les WebViews Tauri ne doivent jamais naviguer à l'intérieur du launcher :
// les liens écrits dans une news s'ouvrent dans le navigateur de l'utilisateur.
if (newsDetailContent) {
  newsDetailContent.addEventListener("click", async (e) => {
    const lien = e.target instanceof Element ? e.target.closest("a[href]") : null;
    if (!lien) return;

    e.preventDefault();
    try {
      await openUrl(lien.href);
    } catch (error) {
      console.warn("Impossible d'ouvrir le lien externe :", error);
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

  // Charge le portefeuille et la boutique de capes.
  chargerBoutiqueCapes();

  // Mettre à jour les news toutes les 5 minutes
  setInterval(chargerNews, 5 * 60 * 1000);
}