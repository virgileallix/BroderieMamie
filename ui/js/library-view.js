// Écran principal : menu de gauche et grille des broderies.
import { api } from "./api.js";
import { showError, toast } from "./dialogs.js";
import { state, counts, allTags, allFolders, visible, viewTitle, upsert, emit, fmtOf, setEntries } from "./state.js";
import { $, el, icon, joinPath, plural, size, prefs, colorCount } from "./util.js";

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

  // Un mot-clé pour toutes les broderies affichées (un dossier, ou le résultat d'une recherche).
  $("btnBulkTag").addEventListener("click", () => {
    const n = visible().length;
    $("bulkText").textContent = `Le mot-clé sera ajouté aux ${plural(n, "broderie affichée", "broderies affichées")} (« ${viewTitle()} »${state.query ? `, recherche « ${state.query} »` : ""}).`;
    $("bulkInput").value = "";
    $("bulkList").replaceChildren(...allTags().map(t => el("option", { value: t.name })));
    $("bulkDlg").showModal();
    $("bulkInput").focus();
  });
  $("bulkCancel").addEventListener("click", () => $("bulkDlg").close());
  $("bulkForm").addEventListener("submit", async e => {
    e.preventDefault();
    const tag = $("bulkInput").value.trim();
    if (!tag) return;
    $("bulkDlg").close();
    try {
      const ids = visible().map(x => x.id);
      const n = await api.addTagTo(ids, tag);
      setEntries((await api.info()).entries);
      toast(n ? `Mot-clé « ${tag} » ajouté à ${plural(n, "broderie")}` : `Ces broderies avaient déjà le mot-clé « ${tag} »`, 4000);
    } catch (err) { showError(err); }
  });
}

/** Lance une recherche (depuis un mot-clé de la fiche, par exemple). */
export function searchFor(text) {
  state.view = "all";
  state.query = text;
  $("q").value = text;
  $("qClear").classList.remove("hidden");
  $("main").scrollTop = 0;
  emit();
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
  const folders = allFolders();
  const folderItem = (view, label, n, extra) => el("button", { class: "nav-item", "data-view": view, title: label },
    icon("folder", "icon nav-ico"), el("span", { class: "nav-label" + (extra ? " nav-label-soft" : "") }, label), el("span", { class: "nav-count" }, n));
  const folderItems = folders.list.map(f => folderItem("dir:" + f.name, f.name, f.n));
  if (folders.list.length && folders.loose) folderItems.push(folderItem("dir:", "Ajoutées une par une", folders.loose, true));
  $("folderNav").replaceChildren(...folderItems);
  $("folderHint").classList.toggle("hidden", folders.list.length > 0);
  if (state.view.startsWith("dir:") && state.view !== "dir:" && !folders.list.some(f => "dir:" + f.name === state.view)) state.view = "all";
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
      : state.view === "all" ? plural(total, "motif") : plural(list.length, "broderie");

  // Bouton « mot-clé pour toutes » : dans un dossier, un mot-clé, ou après une recherche.
  const bulk = list.length > 0 && (state.view.startsWith("dir:") || state.view.startsWith("tag:") || state.view.startsWith("fmt:") || state.query);
  $("btnBulkTag").classList.toggle("hidden", !bulk);
  $("bulkLbl").textContent = state.view.startsWith("dir:") && !state.query
    ? `Ajouter un mot-clé à tout ce dossier (${list.length})`
    : `Ajouter un mot-clé à ces ${plural(list.length, "broderie")}`;

  const grid = $("grid");
  grid.replaceChildren(...list.slice(0, shown).map(card));
  $("more").classList.toggle("hidden", list.length <= shown);

  const empty = $("empty");
  const noResults = total > 0 && list.length === 0;
  empty.classList.toggle("hidden", !(total === 0 || noResults));
  $("emptyActions").classList.toggle("hidden", noResults);
  if (total === 0) {
    $("emptyTitle").textContent = "Votre collection est vide";
    $("emptyText").textContent = "Ajoutez vos fichiers de broderie (.pes, .jef, .vp3, .hus, .vip, .sew, .exp…). Vous pouvez aussi les faire glisser ici, ou choisir un dossier entier : les doublons seront retirés tout seuls, en gardant le PES.";
  } else if (noResults) {
    $("emptyTitle").textContent = state.query ? "Aucune broderie trouvée" : "Rien ici pour l'instant";
    $("emptyText").textContent = state.query
      ? `Aucune broderie ne correspond à « ${state.query} ». Essayez un autre mot, ou vérifiez l'orthographe.`
      : state.view === "fav" ? "Cliquez sur l'étoile d'une broderie pour la retrouver ici."
      : state.view === "fmt:AUTRES" ? "Toutes les broderies sont au format PES : la machine Brother peut toutes les lire."
      : "Aucune broderie dans cette rubrique.";
  }
}

function card(e) {
  const thumb = el("div", { class: "card-thumb" });
  if (e.format) {
    const img = el("img", { src: api.thumbUrl(joinPath(state.thumbs, e.id + ".jpg")), alt: "", loading: "lazy", decoding: "async" });
    img.onerror = () => { thumb.classList.add("empty"); img.replaceWith(el("span", { class: "no-thumb" }, "Aperçu indisponible")); };
    thumb.append(img);
  } else {
    thumb.classList.add("empty");
    thumb.append(el("span", { class: "no-thumb" }, "Aperçu indisponible"));
  }
  const shownColors = e.colors.slice(0, 8), nColors = colorCount(e);
  const body = el("div", { class: "card-body" },
    el("div", { class: "card-name", title: e.name }, e.name),
    el("div", { class: "card-meta" }, e.widthMM ? size(e.widthMM, e.heightMM) : e.originalName),
    e.folder && !state.view.startsWith("dir:") ? el("div", { class: "card-folder", title: "Dossier « " + e.folder + " »" }, icon("folder"), e.folder) : null,
    nColors ? el("div", { class: "card-dots", "aria-label": plural(nColors, "couleur") },
      shownColors.map(c => el("span", { style: `background:${c.color}`, title: c.name || c.color })),
      nColors > 8 ? el("em", {}, "+" + (nColors - 8)) : null) : null);

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
    el("span", { class: "badge" + (fmtOf(e) === "PES" ? "" : " badge-other"), title: fmtOf(e) === "PES" ? "Lisible par la Brother" : "Pas au format PES" }, e.ext || e.format),
    fav);
}
