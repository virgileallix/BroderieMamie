// Recherche intelligente : réglage de la clé Gemini et avancement de l'analyse des motifs.
import { api } from "./api.js";
import { setEntries } from "./state.js";
import { $, plural } from "./util.js";

const STUDIO_URL = "https://aistudio.google.com/app/apikey";
let status = { hasKey: false, running: false, analysed: 0, total: 0, error: null };
let refreshTimer;

export async function initAi() {
  const dlg = $("aiDlg");
  $("btnAi").addEventListener("click", openAi);
  for (const b of dlg.querySelectorAll("[data-close]")) b.addEventListener("click", () => dlg.close());
  $("aiOpenStudio").addEventListener("click", () => api.openUrl(STUDIO_URL));
  $("aiForm").addEventListener("submit", async e => {
    e.preventDefault();
    const key = $("aiKey").value.trim();
    if (!key) return;
    $("aiSubmit").disabled = true;
    $("aiSubmit").textContent = "Vérification…";
    $("aiFormError").classList.add("hidden");
    try {
      setStatus(await api.aiSetKey(key));
      $("aiKey").value = "";
    } catch (err) {
      $("aiFormError").textContent = String(err?.message || err);
      $("aiFormError").classList.remove("hidden");
    } finally {
      $("aiSubmit").disabled = false;
      $("aiSubmit").textContent = "Activer";
    }
  });
  $("aiRemove").addEventListener("click", async () => setStatus(await api.aiRemoveKey()));
  $("aiRetry").addEventListener("click", async () => setStatus(await api.aiStart()));
  $("aiEnable").addEventListener("click", async () => setStatus(await api.aiStart()));
  api.onAiProgress(s => {
    setStatus(s);
    // Les nouveaux mots-clés de l'IA servent tout de suite à la recherche (sans recharger à chaque lot).
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(async () => setEntries((await api.info()).entries), s.running ? 4000 : 0);
  });
  try { setStatus(await api.aiStatus()); } catch { /* pas grave */ }
}

export function openAi() {
  render();
  $("aiDlg").showModal();
  if (!status.hasKey && !status.builtIn) $("aiKey").focus();
}

function setStatus(s) {
  status = s;
  render();
}

function render() {
  const s = status, pct = s.total ? Math.round((s.analysed / s.total) * 100) : 0;
  const card = $("btnAi");
  card.classList.toggle("running", s.running);
  card.classList.toggle("ready", s.hasKey && !s.running && !s.error);
  card.classList.toggle("error", Boolean(s.hasKey && s.error));
  $("aiBar").classList.toggle("hidden", !s.running);
  $("aiBarFill").style.width = pct + "%";
  $("aiLine").textContent = !s.hasKey ? (s.builtIn ? "Désactivée" : "À activer — gratuit")
    : s.error ? "Problème : cliquez pour voir"
    : s.running ? `L'IA regarde les motifs : ${s.analysed} / ${s.total}`
    : s.analysed >= s.total ? `Prête · ${plural(s.analysed, "motif analysé", "motifs analysés")}`
    : `En pause · ${s.analysed} / ${s.total}`;

  $("aiSetup").classList.toggle("hidden", s.hasKey || s.builtIn);
  $("aiOff").classList.toggle("hidden", s.hasKey || !s.builtIn);
  $("aiActive").classList.toggle("hidden", !s.hasKey);
  $("aiRemove").classList.toggle("hidden", !s.hasKey);
  $("aiDlgBar").style.width = pct + "%";
  $("aiDlgState").textContent = s.running
    ? `L'IA regarde les motifs : ${s.analysed} sur ${s.total}. Vous pouvez continuer à utiliser le logiciel.`
    : s.analysed >= s.total ? `C'est prêt ! ${plural(s.total, "motif est analysé", "motifs sont analysés")} : essayez de chercher « ange », « fleur » ou « Noël ».`
    : `${s.analysed} motifs sur ${s.total} analysés. La suite reprendra au prochain démarrage.`;
  $("aiDlgError").classList.toggle("hidden", !s.error);
  $("aiDlgError").textContent = s.error || "";
  $("aiRetryRow").classList.toggle("hidden", !(s.error && !s.running));
}
