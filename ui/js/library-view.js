// Écran principal : menu de gauche et grille des broderies.
import { api } from "./api.js";
import { showError, toast } from "./dialogs.js";
import { state, counts, allTags, visible, viewTitle, upsert, emit } from "./state.js";
import { $, el, icon, joinPath, plural, size, prefs } from "./util.js";

const PAGE = 150;
let shown = PAGE;
let openDetail = () => {};

export function initLibrary({ onOpen }) {
  openDetail = onOpen;

  $("nav").addEventListener("click", e => {
    const b = e.target.closest(".nav-item");
    if (!b) return;
    setView(b.dataset.view);
  });

  let timer;
  $("q").addEventListener("input", () => {
    $("qClear").classList.toggle("hidden", !$("q").value);
    clearTimeout(timer);
    timer = setTimeout(() => { state.query = $("q").value; shown = PAGE; emit(); }, 120);
  });
  $("qClear").addEventListener("click", () => {
    $("q").value = ""; state.query = ""; $("qClear").classList.add("hidden"); emit(); $("q").focus();
  });

  $("sort").value = state.sort = prefs.get("sort", "recent");
  $("sort").addEventListener("change", () => { state.sort = $("sort").value; prefs.set("sort", state.sort); emit(); });

  const setSize = s => {
    document.body.dataset.size = s;
    prefs.set("size", s);
    for (const b of document.querySelectorAll("[data-size]")) if (b !== document.body) b.setAttribute("aria-pressed", b.dataset.size === s);
  };
  setSize(prefs.get("size", "m"));
  for (const b of document.querySelectorAll("button[data-size]")) b.addEventListener("click", () => setSize(b.dataset.size));

  $("btnMore").addEventListener("click", () => { shown += PAGE; render(); });
}

export function setView(view) {
  state.view = view;
  shown = PAGE;
  $("main").scrollTop = 0;
  emit();
}

export function render() {
  renderNav();
  renderGrid();
}

function renderNav() {
  const c = counts();
  for (const n of document.querySelectorAll("[data-count]")) n.textContent = c[n.dataset.count] ?? "";
  const tags = allTags();
  $("tagNav").replaceChildren(...tags.map(t => {
    const v = "tag:" + t.name;
    return el("button", { class: "nav-item", "data-view": v },
      el("span", { class: "nav-hash" }, "#"), el("span", { class: "nav-label" }, t.name), el("span", { class: "nav-count" }, t.n));
  }));
  $("tagHint").classList.toggle("hidden", tags.length > 0);
  // Si le mot-clé affiché n'existe plus, on revient à « Toutes ».
  if (state.view.startsWith("tag:") && !tags.some(t => "tag:" + t.name === state.view)) state.view = "all";
  for (const b of document.querySelectorAll(".nav-item")) b.setAttribute("aria-current", b.dataset.view === state.view);
}

function renderGrid() {
  const list = visible();
  const total = state.entries.length;
  $("viewTitle").textContent = viewTitle();
  $("viewSub").textContent = !total ? "" :
    state.query ? `${plural(list.length, "broderie trouvée", "broderies trouvées")} pour « ${state.query} »`
      : state.view === "all" ? `${plural(total, "motif")}, sans aucun doublon` : plural(list.length, "broderie");

  const grid = $("grid");
  grid.replaceChildren(...list.slice(0, shown).map(card));
  $("more").classList.toggle("hidden", list.length <= shown);

  const empty = $("empty");
  const noResults = total > 0 && list.length === 0;
  empty.classList.toggle("hidden", !(total === 0 || noResults));
  $("emptyActions").classList.toggle("hidden", noResults);
  if (total === 0) {
    $("emptyTitle").textContent = "Votre collection est vide";
    $("emptyText").textContent = "Ajoutez vos fichiers de broderie (.pes, .vp3, .pv3). Vous pouvez aussi les faire glisser ici, ou choisir un dossier entier : les doublons seront retirés tout seuls.";
  } else if (noResults) {
    $("emptyTitle").textContent = state.query ? "Aucune broderie trouvée" : "Rien ici pour l'instant";
    $("emptyText").textContent = state.query
      ? `Aucune broderie ne correspond à « ${state.query} ». Essayez un autre mot, ou vérifiez l'orthographe.`
      : state.view === "fav" ? "Cliquez sur l'étoile d'une broderie pour la retrouver ici." : "Aucune broderie dans cette rubrique.";
  }
}

function card(e) {
  const thumb = el("div", { class: "card-thumb" });
  if (e.format) {
    const img = el("img", { src: api.thumbUrl(joinPath(state.thumbs, e.id + ".png")), alt: "", loading: "lazy", decoding: "async" });
    img.onerror = () => img.replaceWith(el("span", { class: "no-thumb" }, "Aperçu indisponible"));
    thumb.append(img);
  } else {
    thumb.append(el("span", { class: "no-thumb" }, "Aperçu indisponible"));
  }
  const shownColors = e.colors.slice(0, 8);
  const body = el("div", { class: "card-body" },
    el("div", { class: "card-name", title: e.name }, e.name),
    el("div", { class: "card-meta" }, e.widthMM ? size(e.widthMM, e.heightMM) : e.originalName),
    e.colors.length ? el("div", { class: "card-dots", "aria-label": plural(e.colors.length, "couleur") },
      shownColors.map(c => el("span", { style: `background:${c.color}`, title: c.name || c.color })),
      e.colors.length > 8 ? el("em", {}, "+" + (e.colors.length - 8)) : null) : null);

  const fav = el("button", {
    class: "card-fav" + (e.favorite ? " on" : ""),
    "aria-label": e.favorite ? "Retirer des favoris" : "Mettre en favori",
    title: e.favorite ? "Retirer des favoris" : "Mettre en favori",
    onclick: async ev => {
      ev.stopPropagation();
      try {
        upsert(await api.update(e.id, { favorite: !e.favorite }));
        toast(e.favorite ? "Retirée des favoris" : "Ajoutée aux favoris");
      } catch (err) { showError(err); }
    },
  }, icon(e.favorite ? "star-fill" : "star"));

  return el("article", { class: "card" },
    el("button", { class: "card-open", "aria-label": `Ouvrir ${e.name}`, onclick: () => openDetail(e.id) }, thumb, body),
    el("span", { class: "badge" }, e.ext === "PV3" ? "PV3" : e.format || e.ext),
    fav);
}
