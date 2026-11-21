// Ajout de broderies : barre de progression puis compte rendu (ajoutées, doublons, ignorées).
import { api } from "./api.js";
import { showError } from "./dialogs.js";
import { openDuplicates } from "./duplicates-view.js";
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
  $("similarBtn").addEventListener("click", () => { $("importDlg").close(); openDuplicates(); });
  $("importDlg").addEventListener("cancel", e => { if (busy) e.preventDefault(); });
}

export async function addFiles() {
  try { const paths = await api.pickFiles(); if (paths.length) await importPaths(paths); } catch (e) { showError(e); }
}

export async function addFolder() {
  try {
    const paths = await api.pickFolders("Choisir des dossiers de broderies (Ctrl + clic pour en prendre plusieurs)");
    if (paths.length) await importPaths(paths);
  } catch (e) { showError(e); }
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
  const n = r.added.length, d = r.duplicates.length, sim = r.similar.length;
  $("iTitle").textContent = n ? (n > 1 ? "Les broderies sont rangées !" : "La broderie est rangée !")
    : d ? "Tout était déjà dans la collection" : "Aucune broderie trouvée";
  $("iRunning").classList.add("hidden");
  $("iDone").classList.remove("hidden");
  setStat("sAdded", "sAddedLbl", n, "ajoutée", "ajoutées");
  setStat("sDup", "sDupLbl", d, "déjà là, pas ajoutée", "déjà là, pas ajoutées");
  setStat("sSkip", "sSkipLbl", r.ignored, "autre fichier ignoré", "autres fichiers ignorés");
  $("dupDetails").classList.toggle("hidden", !d);
  $("dupList").replaceChildren(...r.duplicates.map(x => el("li", {}, `${x.file} → déjà là sous le nom « ${x.sameAs} »`)));
  $("dupDetails").querySelector("summary").textContent = "Voir les fichiers déjà là";
  $("similarBox").classList.toggle("hidden", !sim);
  $("similarText").textContent = sim
    ? `${plural(sim, "broderie ajoutée ressemble", "broderies ajoutées ressemblent")} à des motifs déjà rangés (même motif dans un autre format, par exemple). Vous pouvez les comparer et choisir lesquelles garder.`
    : "";
  $("probDetails").classList.toggle("hidden", !r.problems.length);
  $("probList").replaceChildren(...r.problems.map(p => el("li", {}, p)));
  $("iDone").querySelector("[data-close]").focus();
  if (!n && !d) $("dupDetails").insertAdjacentElement("beforebegin", el("p", { class: "muted" },
    "Les broderies sont les fichiers .pes, .jef, .vp3, .hus, .vip, .sew, .exp, .dst… " + plural(r.ignored, "autre fichier a été ignoré", "autres fichiers ont été ignorés") + "."));
}
