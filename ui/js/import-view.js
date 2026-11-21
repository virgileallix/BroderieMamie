// Ajout de broderies : barre de progression puis compte rendu (ajoutées, doublons, ignorées).
import { api } from "./api.js";
import { showError } from "./dialogs.js";
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
  const n = r.added.length, d = r.duplicates.length;
  $("iTitle").textContent = n ? (n > 1 ? "Les broderies sont rangées !" : "La broderie est rangée !")
    : d ? "Tout était déjà dans la collection" : "Aucune broderie trouvée";
  $("iRunning").classList.add("hidden");
  $("iDone").classList.remove("hidden");
  $("sAdded").textContent = n;
  $("sAddedLbl").textContent = n > 1 ? "ajoutées" : "ajoutée";
  $("sDup").textContent = d;
  $("sDupLbl").textContent = d > 1 ? "doublons retirés" : "doublon retiré";
  $("sSkip").textContent = r.ignored;
  $("sSkipLbl").textContent = r.ignored > 1 ? "autres fichiers ignorés" : "autre fichier ignoré";
  $("dupDetails").classList.toggle("hidden", !d);
  $("dupList").replaceChildren(...r.duplicates.map(x => el("li", {}, `${x.file} → déjà là sous le nom « ${x.sameAs} »`)));
  $("probDetails").classList.toggle("hidden", !r.problems.length);
  $("probList").replaceChildren(...r.problems.map(p => el("li", {}, p)));
  $("iDone").querySelector("[data-close]").focus();
  if (!n && !d) $("dupDetails").insertAdjacentElement("beforebegin", el("p", { class: "muted" },
    "Seuls les fichiers .pes, .vp3, .pv3 et .pec sont des broderies. " + plural(r.ignored, "autre fichier a été ignoré", "autres fichiers ont été ignorés") + "."));
}
