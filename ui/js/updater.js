// Mise à jour automatique : au démarrage, on regarde s'il existe une nouvelle version sur GitHub.
import { confirmBox } from "./dialogs.js";
import { $ } from "./util.js";

const T = window.__TAURI__;

export async function checkForUpdate() {
  if (!T?.updater) return;
  let update;
  try {
    update = await T.updater.check();
  } catch (e) {
    console.warn("Vérification des mises à jour impossible :", e); // pas d'internet : on ne dérange pas
    return;
  }
  if (!update) return;
  const ok = await confirmBox(
    "Une nouvelle version est disponible",
    `La version ${update.version} des « Broderies de Mamie » est prête. Voulez-vous l'installer maintenant ? ` +
      "Cela prend environ une minute, et vos broderies ne bougent pas.",
    "Oui, mettre à jour",
    "Plus tard",
  );
  if (!ok) return;

  const dlg = $("importDlg");
  $("iTitle").textContent = "Mise à jour en cours…";
  $("iRunning").classList.remove("hidden");
  $("iDone").classList.add("hidden");
  $("iBar").style.width = "0%";
  $("iFile").textContent = "Téléchargement de la nouvelle version…";
  dlg.addEventListener("cancel", e => e.preventDefault(), { once: true });
  dlg.showModal();
  let total = 0, got = 0;
  try {
    await update.downloadAndInstall(ev => {
      if (ev.event === "Started") total = ev.data.contentLength || 0;
      if (ev.event === "Progress") {
        got += ev.data.chunkLength;
        if (total) $("iBar").style.width = Math.round((got / total) * 100) + "%";
      }
      if (ev.event === "Finished") $("iFile").textContent = "Installation… le logiciel va redémarrer tout seul.";
    });
    await T.process.relaunch();
  } catch (e) {
    dlg.close();
    confirmBox("La mise à jour n'a pas marché", "Ce n'est pas grave : le logiciel continue de fonctionner. On réessaiera au prochain démarrage.", "D'accord", "Fermer");
  }
}
