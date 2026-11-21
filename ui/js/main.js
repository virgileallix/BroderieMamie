// Point d'entrée de l'interface.
import { api, available } from "./api.js";
import { initDetail, openDetail } from "./detail-view.js";
import { confirmBox, showError, toast } from "./dialogs.js";
import { addFiles, addFolder, importPaths, initImport } from "./import-view.js";
import { initLibrary, render } from "./library-view.js";
import { state, setEntries, onChange } from "./state.js";
import { $ } from "./util.js";
import { checkForUpdate } from "./updater.js";

async function start() {
  if (!available) {
    document.body.innerHTML = "<p style='padding:40px;font-size:22px'>Ouvrez « Les broderies de Mamie » depuis l'application installée.</p>";
    return;
  }
  initLibrary({ onOpen: openDetail });
  initDetail();
  initImport();
  onChange(render);

  $("btnAdd").addEventListener("click", addFiles);
  for (const b of document.querySelectorAll("[data-action=add-files]")) b.addEventListener("click", addFiles);
  for (const b of document.querySelectorAll("[data-action=add-folder]")) b.addEventListener("click", addFolder);
  $("btnFolder").addEventListener("click", () => api.reveal(state.root).catch(showError));
  $("btnBackup").addEventListener("click", backup);

  // Glisser-déposer des fichiers depuis l'Explorateur
  api.onDragDrop(p => {
    if (p.type === "enter" || p.type === "over") $("dropOverlay").classList.remove("hidden");
    else $("dropOverlay").classList.add("hidden");
    if (p.type === "drop" && p.paths?.length) importPaths(p.paths);
  });

  // Raccourcis : Ctrl+F pour chercher
  document.addEventListener("keydown", e => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") { e.preventDefault(); $("q").focus(); $("q").select(); }
  });

  try {
    const info = await api.info();
    state.root = info.root;
    state.thumbs = info.thumbs;
    setEntries(info.entries);
  } catch (e) { showError(e); render(); }
  setTimeout(checkForUpdate, 2500);
}

async function backup() {
  if (!state.entries.length) { toast("La collection est vide, il n'y a rien à sauvegarder."); return; }
  try {
    const date = new Date().toLocaleDateString("fr-FR").replaceAll("/", "-");
    const dest = await api.saveZip(`Broderies de Mamie ${date}.zip`);
    if (!dest) return;
    toast("Sauvegarde en cours…", 20000);
    const n = await api.backup(dest);
    toast(`Sauvegarde terminée : ${n} broderies dans un seul fichier ✔`, 5000);
  } catch (e) { showError(e); }
}

window.addEventListener("error", e => showError(e.error || e.message));
window.addEventListener("unhandledrejection", e => showError(e.reason));
start();
export { confirmBox };
