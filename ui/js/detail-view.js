// Fiche d'une broderie : grand aperçu animé + informations, mots-clés, actions.
import { api } from "./api.js";
import { confirmBox, showError, toast } from "./dialogs.js";
import { Viewer, bestHoop } from "./renderer.js";
import { state, upsert, removeEntry, allTags, fmtOf } from "./state.js";
import { $, el, icon, joinPath, norm, plural, size, cm, prefs } from "./util.js";

let viewer;
let entry = null;
let token = 0;
let anim = 0;
let speed = 1;

const dlg = () => $("detail");

export function initDetail() {
  viewer = new Viewer($("cv"));
  viewer.onCurrentBlock = highlightThread;

  dlg().addEventListener("close", () => { stopPlay(); entry = null; viewer.set(null); });
  for (const b of dlg().querySelectorAll("[data-close]")) b.addEventListener("click", () => dlg().close());

  const setBg = bg => {
    $("stage").className = "stage bg-" + bg;
    prefs.set("bg", bg);
    for (const b of document.querySelectorAll("[data-bg]")) b.setAttribute("aria-pressed", b.dataset.bg === bg);
  };
  setBg(prefs.get("bg", "tissu"));
  for (const b of document.querySelectorAll("[data-bg]")) b.addEventListener("click", () => setBg(b.dataset.bg));

  const setSpeed = v => {
    speed = +v;
    for (const b of document.querySelectorAll("[data-speed]")) b.setAttribute("aria-pressed", b.dataset.speed === String(v));
  };
  setSpeed(1);
  for (const b of document.querySelectorAll("[data-speed]")) b.addEventListener("click", () => setSpeed(b.dataset.speed));

  $("zIn").onclick = () => viewer.zoomBy(1.4);
  $("zOut").onclick = () => viewer.zoomBy(1 / 1.4);
  $("zFit").onclick = () => { viewer.zoom = 1; viewer.panX = viewer.panY = 0; viewer.draw(); };
  $("hoopBtn").onclick = () => {
    viewer.showHoop = !viewer.showHoop;
    $("hoopBtn").setAttribute("aria-pressed", viewer.showHoop);
    viewer.zoom = 1; viewer.panX = viewer.panY = 0; viewer.draw();
  };

  $("play").onclick = () => (anim ? stopPlay() : startPlay());
  $("prog").addEventListener("input", () => {
    stopPlay();
    setProgress(+$("prog").value);
  });

  // Renommer
  $("btnRename").onclick = () => {
    $("renameForm").classList.remove("hidden");
    $("dTitle").parentElement.classList.add("hidden");
    $("renameInput").value = entry.name;
    $("renameInput").focus();
    $("renameInput").select();
  };
  const closeRename = () => { $("renameForm").classList.add("hidden"); $("dTitle").parentElement.classList.remove("hidden"); };
  $("renameInput").addEventListener("keydown", e => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeRename(); } });
  $("renameForm").addEventListener("submit", async e => {
    e.preventDefault();
    const name = $("renameInput").value.trim();
    closeRename();
    if (!name || name === entry.name) return;
    await save({ name }, "Nom modifié");
  });

  // Mots-clés
  $("tagForm").addEventListener("submit", async e => {
    e.preventDefault();
    const words = $("tagInput").value.split(",").map(s => s.trim()).filter(Boolean);
    $("tagInput").value = "";
    const fresh = words.filter(w => !entry.tags.some(t => norm(t) === norm(w)));
    if (fresh.length) await save({ tags: [...entry.tags, ...fresh] }, fresh.length > 1 ? "Mots-clés ajoutés" : "Mot-clé ajouté");
  });

  $("btnFav").onclick = () => save({ favorite: !entry.favorite }, entry.favorite ? "Retirée des favoris" : "Ajoutée aux favoris ★");
  $("btnReveal").onclick = () => api.reveal(joinPath(state.root, joinPath("Motifs", entry.fileName))).catch(showError);

  $("btnExport").onclick = async () => {
    try {
      const [dir] = await api.pickFolder("Où copier la broderie ? (choisissez votre clé USB)");
      if (!dir) return;
      await api.exportTo(entry.id, dir);
      toast("Broderie copiée ✔ Vous pouvez retirer la clé et l'utiliser sur la machine.", 5000);
    } catch (e) { showError(e); }
  };

  $("btnDelete").onclick = async () => {
    const e = entry;
    const ok = await confirmBox("Supprimer cette broderie ?",
      `« ${e.name} » sera retirée de la collection. Le fichier est rangé dans le dossier « Corbeille », au cas où.`);
    if (!ok) return;
    try {
      await api.remove(e.id);
      removeEntry(e.id);
      dlg().close();
      toast("Broderie supprimée");
    } catch (err) { showError(err); }
  };

  document.addEventListener("keydown", e => {
    if (!dlg().open || e.target.closest("input")) return;
    if (e.key === " ") { e.preventDefault(); $("play").click(); }
    if (e.key === "+" || e.key === "=") viewer.zoomBy(1.4);
    if (e.key === "-") viewer.zoomBy(1 / 1.4);
  });
}

async function save(update, message) {
  try {
    entry = await api.update(entry.id, update);
    upsert(entry);
    fillInfo();
    if (message) toast(message);
  } catch (e) { showError(e); }
}

export async function openDetail(id) {
  entry = state.entries.find(e => e.id === id);
  if (!entry) return;
  const my = ++token;
  stopPlay();
  viewer.set(null);
  viewer.showHoop = false;
  $("hoopBtn").setAttribute("aria-pressed", "false");
  $("stageError").classList.add("hidden");
  $("prog").value = 1000;
  fillInfo();
  if (!dlg().open) dlg().showModal();
  $("play").focus();
  try {
    const d = await api.design(id);
    if (my !== token) return;
    viewer.set(d);
    fillThreads(d);
    updateLabel();
  } catch (e) {
    $("stageError").classList.remove("hidden");
    $("threads").replaceChildren();
  }
}

function fillInfo() {
  const e = entry;
  $("dTitle").textContent = e.name;
  $("dFile").textContent = `Fichier « ${e.fileName} »` + (e.originalName && e.originalName !== e.fileName ? ` — importé depuis « ${e.originalName} »` : "");
  $("dNotice").classList.toggle("hidden", fmtOf(e) === "PES");
  const hoop = e.widthMM ? bestHoop(e.widthMM, e.heightMM) : null;
  const facts = e.format ? [
    ["Taille", size(e.widthMM, e.heightMM), true],
    ["Points", e.stitches.toLocaleString("fr-FR")],
    ["Couleurs", String(e.colors.length)],
    ["Cadre conseillé", hoop ? `${cm(hoop[0])} × ${cm(hoop[1])} cm` : "très grand"],
    ["Format", e.ext === e.format ? e.format : `${e.format} (.${e.ext.toLowerCase()})`],
  ] : [["Aperçu", "Ce fichier n'a pas pu être lu", true], ["Format", e.ext]];
  facts.push(["Ajoutée le", new Date(e.addedAt).toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" }), true]);
  $("facts").replaceChildren(...facts.map(([k, v, wide]) =>
    el("div", { class: "fact" + (wide ? " wide" : "") }, el("span", {}, k), el("strong", {}, v))));

  $("tags").replaceChildren(...e.tags.map(t => el("span", { class: "tag" }, t,
    el("button", { "aria-label": `Retirer le mot-clé ${t}`, title: "Retirer", onclick: () => save({ tags: e.tags.filter(x => x !== t) }) }, icon("x")))));
  $("tagList").replaceChildren(...allTags().filter(t => !e.tags.some(x => norm(x) === norm(t.name))).map(t => el("option", { value: t.name })));

  $("btnFav").setAttribute("aria-pressed", e.favorite);
  $("btnFav").replaceChildren(icon(e.favorite ? "star-fill" : "star"), e.favorite ? "Favori" : "Favori");
  $("btnFav").title = e.favorite ? "Retirer des favoris" : "Mettre en favori";
}

function fillThreads(d) {
  $("colorCount").textContent = d.blocks.length ? `(${d.blocks.length})` : "";
  $("threads").replaceChildren(...d.blocks.map((b, i) =>
    el("li", { "data-i": i },
      el("span", { class: "spool", style: `background:${b.color}` }),
      el("span", { class: "thread-name" }, b.name || "Fil " + (i + 1)),
      el("span", { class: "thread-code" }, b.color.toUpperCase()))));
}

let lastHi = -2;
function highlightThread(i) {
  if (i === lastHi) return;
  lastHi = i;
  for (const li of $("threads").children) li.classList.toggle("current", +li.dataset.i === i);
}

/* ---------- Animation « Voir la broderie se faire » ---------- */
function total() { return viewer.design?.stitchCount || 0; }

function setProgress(v) {
  $("prog").value = v;
  viewer.limit = v >= 1000 ? Infinity : Math.round(total() * v / 1000);
  viewer.draw();
  updateLabel();
}

function updateLabel() {
  const v = +$("prog").value, n = total();
  $("progLbl").textContent = !n ? "" : v >= 1000 ? `${plural(n, "point")} au total`
    : `point ${Math.round(n * v / 1000).toLocaleString("fr-FR")} sur ${n.toLocaleString("fr-FR")}`;
}

function startPlay() {
  if (!total()) return;
  if (+$("prog").value >= 1000) setProgress(0);
  $("playIco").firstElementChild.setAttribute("href", "#i-pause");
  $("play").setAttribute("aria-label", "Mettre en pause");
  $("playTitle").textContent = "La broderie se fait…";
  const duration = Math.min(40000, Math.max(8000, total() * 1.5)); // en vitesse normale
  let last = performance.now();
  const step = t => {
    const v = Math.min(1000, +$("prog").value + ((t - last) / duration) * 1000 * speed);
    last = t;
    setProgress(v);
    if (v >= 1000) { stopPlay(); return; }
    anim = requestAnimationFrame(step);
  };
  anim = requestAnimationFrame(step);
}

function stopPlay() {
  if (anim) cancelAnimationFrame(anim);
  anim = 0;
  $("playIco").firstElementChild.setAttribute("href", "#i-play");
  $("play").setAttribute("aria-label", "Voir la broderie se faire");
  $("playTitle").textContent = +$("prog").value >= 1000 ? "Voir la broderie se faire" : "En pause";
}
