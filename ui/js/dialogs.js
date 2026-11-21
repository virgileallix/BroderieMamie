import { $, icon } from "./util.js";

let toastTimer;
export function toast(message, ms = 3200) {
  const t = $("toast");
  t.replaceChildren(icon("check"), message);
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), ms);
}

export function confirmBox(title, text, yes = "Oui, supprimer", no = "Non, garder") {
  const dlg = $("confirmDlg");
  $("cTitle").textContent = title;
  $("cText").textContent = text;
  $("cYes").textContent = yes;
  $("cNo").textContent = no;
  return new Promise(resolve => {
    const done = v => { dlg.close(); resolve(v); };
    $("cYes").onclick = () => done(true);
    $("cNo").onclick = () => done(false);
    dlg.oncancel = e => { e.preventDefault(); done(false); };
    dlg.showModal();
    $("cNo").focus();
  });
}

export function showError(e) {
  console.error(e);
  toast("Oups : " + (e?.message || e), 6000);
}
