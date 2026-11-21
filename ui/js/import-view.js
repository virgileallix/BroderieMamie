// Ajout de broderies : barre de progression puis compte rendu (ajoutées, doublons, ignorées).
import { api } from "./api.js";
import { confirmBox, showError } from "./dialogs.js";
import { state, setEntries } from "./state.js";
import { $, el, plural } from "./util.js";

let busy = false;

export function initImport() {
  api.onProgress(p => {
    if (!busy) return;
    $("iBar").style.width = (p.total ? (p.done / p.total) * 100 : 0) + "%";
    $("iFile").textContent = p.total ? `${Math.min(p.done + 1, p.total)} sur ${p.total} — ${p.file || "c'est presque fini…"}` : "Recherche des fichiers…";
  });
  for (const b of $("importDlg").querySelectorAll("[data-close]")) b.addEventListener("click", () => $("importDlg").close());
  $("importDlg").addEventListener("cancel", e => { if (busy) e.preventDefault(); });
}

export async function addFiles() {
  try { const paths = await api.pickFiles(); if (paths.length) await importPaths(paths); } catch (e) { showError(e); }
}

export async function addFolder() {
  try { const paths = await api.pickFolder("Choisir un dossier de broderies"); if (paths.length) await importPaths(paths); } catch (e) { showError(e); }
}

/** Cherche les doublons déjà présents dans la collection et les range dans la Corbeille. */
export async function findDuplicates() {
  if (busy) return;
  const ok = await confirmBox("Chercher les doublons ?",
    "Le logiciel va regarder toute la collection. Quand un motif est en plusieurs exemplaires (même dessin, même taille), " +
    "il garde la version PES et range les autres dans la Corbeille. Les mots-clés et les étoiles sont conservés.",
    "Oui, chercher", "Non, pas maintenant");
  if (!ok) return;
  busy = true;
  const dlg = $("importDlg");
  $("iTitle").textContent = "Recherche des doublons…";
  $("iRunning").classList.remove("hidden");
  $("iDone").classList.add("hidden");
  $("iBar").style.width = "100%";
  $("iFile").textContent = "Comparaison des motifs entre eux…";
  dlg.showModal();
  try {
    const r = await api.dedupe();
    const info = await api.info();
    setEntries(info.entries);
    const n = r.removed.length;
    $("iTitle").textContent = n ? "La collection est rangée !" : "Aucun doublon trouvé";
    $("iRunning").classList.add("hidden");
    $("iDone").classList.remove("hidden");
    setStat("sAdded", "sAddedLbl", r.kept, "broderie gardée", "broderies gardées");
    setStat("sDup", "sDupLbl", n, "doublon retiré", "doublons retirés");
    $("sSkip").parentElement.classList.add("hidden");
    $("dupDetails").classList.toggle("hidden", !n);
    $("dupList").replaceChildren(...r.removed.map(x => el("li", {}, `${x.file} → doublon de « ${x.sameAs} »`)));
    $("probDetails").classList.add("hidden");
    $("iDone").querySelector("[data-close]").focus();
  } catch (e) {
    dlg.close();
    showError(e);
  } finally {
    busy = false;
  }
}

function setStat(numId, lblId, n, one, many) {
  $(numId).textContent = n;
  $(lblId).textContent = n > 1 ? many : one;
  $(numId).parentElement.classList.remove("hidden");
}

export async function importPaths(paths) {
  if (busy || !paths.length) return;
  busy = true;
  const dlg = $("importDlg");
  $("iTitle").textContent = "Ajout des broderies…";
  $("iRunning").classList.remove("hidden");
  $("iDone").classList.add("hidden");
  $("iBar").style.width = "0%";
  $("iFile").textContent = "Recherche des fichiers…";
  dlg.showModal();
  try {
    const r = await api.importPaths(paths);
    const info = await api.info();
    state.root = info.root; state.thumbs = info.thumbs;
    setEntries(info.entries);
    showReport(r);
  } catch (e) {
    dlg.close();
    showError(e);
  } finally {
    busy = false;
  }
}

function showReport(r) {
  const n = r.added.length, d = r.duplicates.length + r.replaced.length;
  $("iTitle").textContent = n ? (n > 1 ? "Les broderies sont rangées !" : "La broderie est rangée !")
    : d ? "Tout était déjà dans la collection" : "Aucune broderie trouvée";
  $("iRunning").classList.add("hidden");
  $("iDone").classList.remove("hidden");
  setStat("sAdded", "sAddedLbl", n, "ajoutée", "ajoutées");
  setStat("sDup", "sDupLbl", d, "doublon retiré", "doublons retirés");
  setStat("sSkip", "sSkipLbl", r.ignored, "autre fichier ignoré", "autres fichiers ignorés");
  $("dupDetails").classList.toggle("hidden", !d);
  $("dupList").replaceChildren(
    ...r.replaced.map(x => el("li", {}, `${x.file} a remplacé « ${x.sameAs} » (on garde la version PES)`)),
    ...r.duplicates.map(x => el("li", {}, `${x.file} → déjà là sous le nom « ${x.sameAs} »`)));
  $("probDetails").classList.toggle("hidden", !r.problems.length);
  $("probList").replaceChildren(...r.problems.map(p => el("li", {}, p)));
  $("iDone").querySelector("[data-close]").focus();
  if (!n && !d) $("dupDetails").insertAdjacentElement("beforebegin", el("p", { class: "muted" },
    "Les broderies sont les fichiers .pes, .jef, .vp3, .hus, .vip, .sew, .exp, .dst… " + plural(r.ignored, "autre fichier a été ignoré", "autres fichiers ont été ignorés") + "."));
}
