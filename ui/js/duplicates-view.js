// Vérification des doublons : on montre les motifs en plusieurs exemplaires côte à côte,
// et c'est Mamie qui choisit ce qui va à la corbeille. Rien n'est supprimé sans son accord.
import { api } from "./api.js";
import { showError, toast } from "./dialogs.js";
import { state, setEntries } from "./state.js";
import { $, el, icon, joinPath, plural, size } from "./util.js";

let groups = [];
const removeSet = new Set(); // identifiants cochés « à la corbeille »

export function initDuplicates() {
  const dlg = $("dupSheet");
  for (const b of dlg.querySelectorAll("[data-close]")) b.addEventListener("click", () => dlg.close());
  $("dupApply").addEventListener("click", apply);
}

export async function openDuplicates() {
  const dlg = $("dupSheet");
  $("dupBody").replaceChildren(el("div", { class: "dup-empty" }, el("p", { class: "muted" }, "Comparaison des motifs…")));
  $("dupFoot").classList.add("hidden");
  $("dupSub").textContent = "";
  if (!dlg.open) dlg.showModal();
  try {
    groups = await api.findDuplicates();
  } catch (e) {
    dlg.close();
    showError(e);
    return;
  }
  removeSet.clear();
  // Proposition : on garde le premier (le PES, sinon le plus ancien), les autres à la corbeille.
  for (const g of groups) for (const id of g.ids.slice(1)) removeSet.add(id);
  render();
}

const entry = id => state.entries.find(e => e.id === id);

function render() {
  if (!groups.length) {
    $("dupSub").textContent = "";
    $("dupFoot").classList.add("hidden");
    $("dupBody").replaceChildren(el("div", { class: "dup-empty" },
      el("img", { src: "img/cerceau.svg", alt: "", width: 140, height: 140 }),
      el("h3", {}, "Aucun doublon"),
      el("p", { class: "muted" }, "Chaque motif n'est qu'une fois dans la collection.")));
    return;
  }
  $("dupSub").textContent =
    `${plural(groups.length, "motif est", "motifs sont")} en plusieurs exemplaires (même taille, même dessin). ` +
    "Le logiciel propose de garder le PES. Touchez une broderie pour changer : « Je garde » ou « À la corbeille ».";
  $("dupFoot").classList.remove("hidden");
  $("dupBody").replaceChildren(...groups.map((g, gi) => groupPanel(g, gi)));
  updateCount();
}

function groupPanel(g, gi) {
  const name = entry(g.ids[0])?.name || "Motif";
  return el("section", { class: "dup-group" },
    el("div", { class: "dup-group-head" },
      el("h3", {}, `${gi + 1}. ${name}`),
      el("button", { class: "link-btn", onclick: () => { for (const id of g.ids) removeSet.delete(id); render(); } },
        icon("check"), "Tout garder")),
    el("div", { class: "dup-items" }, g.ids.map((id, i) => item(g, id, i))));
}

function item(g, id, i) {
  const e = entry(id);
  if (!e) return null;
  const removing = removeSet.has(id);
  const fmt = e.format || e.ext;
  return el("div", { class: "dup-item" + (removing ? " remove" : "") },
    el("div", { class: "dup-thumb" },
      el("img", { src: api.thumbUrl(joinPath(state.thumbs, e.id + ".jpg")), alt: "" }),
      el("span", { class: "badge" + (["PES", "PEC"].includes(fmt) ? "" : " badge-other") }, e.ext || fmt),
      i === 0 ? el("span", { class: "dup-advice" }, "Conseillé") : null),
    el("div", { class: "dup-info" },
      el("strong", {}, e.name),
      el("span", {}, e.widthMM ? `${size(e.widthMM, e.heightMM)} · ${plural(e.stitches, "point")}` : ""),
      el("span", {}, `Fichier « ${e.fileName.split("/").pop()} »`),
      e.folder ? el("span", {}, `Dossier « ${e.folder} »`) : null,
      e.tags.length ? el("span", {}, "Mots-clés : " + e.tags.join(", ")) : null),
    el("button", {
      class: "dup-toggle",
      "aria-pressed": removing,
      onclick: () => toggle(g, id),
    }, removing ? [icon("trash"), "À la corbeille"] : [icon("check"), "Je garde"]));
}

function toggle(g, id) {
  if (removeSet.has(id)) {
    removeSet.delete(id);
  } else {
    if (g.ids.filter(x => !removeSet.has(x)).length <= 1) {
      toast("Il faut garder au moins un exemplaire de chaque motif.");
      return;
    }
    removeSet.add(id);
  }
  render();
}

function updateCount() {
  const n = removeSet.size;
  $("dupCount").textContent = n ? `${plural(n, "broderie sera mise", "broderies seront mises")} à la corbeille` : "Rien ne sera mis à la corbeille";
  $("dupApplyLbl").textContent = n ? `Mettre ${n} à la corbeille` : "Valider";
}

async function apply() {
  // Pour chaque groupe : la broderie gardée en premier reçoit les mots-clés et l'étoile des autres.
  const choices = groups.map(g => {
    const keep = g.ids.filter(id => !removeSet.has(id));
    return { keep, remove: g.ids.filter(id => removeSet.has(id)) };
  });
  try {
    const n = await api.removeDuplicates(choices);
    const info = await api.info();
    setEntries(info.entries);
    $("dupSheet").close();
    toast(n ? `${plural(n, "doublon rangé", "doublons rangés")} dans la corbeille ✔` : "C'est noté : ces broderies sont toutes gardées.", 4500);
  } catch (e) {
    showError(e);
  }
}
