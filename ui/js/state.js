// État de l'application : la liste des broderies et ce qui est affiché.
import { norm } from "./util.js";

const RECENT_DAYS = 30;

export const state = {
  root: "",
  thumbs: "",
  entries: [],
  view: "all", // all | fav | recent | fmt:PES | fmt:VP3 | tag:<mot-clé>
  query: "",
  sort: "recent",
  listeners: new Set(),
};

export function onChange(fn) { state.listeners.add(fn); }
export function emit() { for (const fn of state.listeners) fn(); }

export function setEntries(list) { state.entries = list; emit(); }
export function upsert(entry) {
  const i = state.entries.findIndex(e => e.id === entry.id);
  if (i >= 0) state.entries[i] = entry; else state.entries.push(entry);
  emit();
}
export function removeEntry(id) { state.entries = state.entries.filter(e => e.id !== id); emit(); }

const isRecent = e => Date.now() - e.addedAt < RECENT_DAYS * 86400000;
/** « PES » si la machine Brother sait le lire, sinon « AUTRES ». */
export const fmtOf = e => (["PES", "PEC"].includes(e.format || e.ext) ? "PES" : "AUTRES");

export function inView(e, view = state.view) {
  if (view === "fav") return e.favorite;
  if (view === "recent") return isRecent(e);
  if (view.startsWith("fmt:")) return fmtOf(e) === view.slice(4);
  if (view.startsWith("tag:")) return e.tags.some(t => norm(t) === norm(view.slice(4)));
  if (view === "dir:") return !e.folder;
  if (view.startsWith("dir:")) return e.folder === view.slice(4);
  return true;
}

export function counts() {
  const c = { all: state.entries.length, fav: 0, recent: 0, "fmt:PES": 0, "fmt:AUTRES": 0 };
  for (const e of state.entries) {
    if (e.favorite) c.fav++;
    if (isRecent(e)) c.recent++;
    const f = "fmt:" + fmtOf(e);
    if (f in c) c[f]++;
  }
  return c;
}

/** Tous les mots-clés utilisés, avec leur nombre, triés par ordre alphabétique. */
export function allTags() {
  const m = new Map();
  for (const e of state.entries) for (const t of e.tags) {
    const k = norm(t);
    const cur = m.get(k) || { name: t, n: 0 };
    cur.n++;
    m.set(k, cur);
  }
  return [...m.values()].sort((a, b) => a.name.localeCompare(b.name, "fr", { sensitivity: "base" }));
}

/** Les dossiers (un par dossier ou .zip importé), avec leur nombre de broderies. */
export function allFolders() {
  const m = new Map();
  let loose = 0;
  for (const e of state.entries) {
    if (e.folder) m.set(e.folder, (m.get(e.folder) || 0) + 1);
    else loose++;
  }
  const list = [...m].map(([name, n]) => ({ name, n })).sort((a, b) => a.name.localeCompare(b.name, "fr", { numeric: true, sensitivity: "base" }));
  return { list, loose };
}

/** Découpe un texte en mots sans accents, au singulier approximatif (« Anges » → « ange »). */
export const words = s => norm(s).split(/[^a-z0-9]+/).filter(Boolean).map(w => (w.length > 3 ? w.replace(/[sx]$/, "") : w));

const wordCache = new WeakMap();
function entryWords(e) {
  let w = wordCache.get(e);
  if (!w) {
    w = words([e.name, e.originalName, e.label, e.folder, e.tags.join(" "), e.ext, e.format,
      e.aiDescription, (e.aiKeywords || []).join(" "), e.colors.map(c => c.name).join(" ")].join(" "));
    wordCache.set(e, w);
  }
  return w;
}

/** Les broderies à afficher, filtrées par la vue et la recherche, puis triées.
 *  Chaque mot cherché doit être le début d'un mot de la broderie : « ange » trouve « ange » et
 *  « angelot », mais pas « orange ». */
export function visible() {
  const q = words(state.query);
  const list = state.entries.filter(e => {
    if (!inView(e)) return false;
    if (!q.length) return true;
    const w = entryWords(e);
    return q.every(x => w.some(y => y.startsWith(x)));
  });
  const area = e => e.widthMM * e.heightMM;
  const byName = (a, b) => a.name.localeCompare(b.name, "fr", { numeric: true, sensitivity: "base" });
  const sorters = {
    recent: (a, b) => b.addedAt - a.addedAt,
    name: byName,
    big: (a, b) => area(b) - area(a),
    small: (a, b) => area(a) - area(b),
    colors: (a, b) => a.colors.length - b.colors.length || byName(a, b),
  };
  return list.sort(sorters[state.sort] || sorters.recent);
}

export function viewTitle(view = state.view) {
  if (view === "fav") return "Mes favoris";
  if (view === "recent") return "Ajoutées récemment";
  if (view === "fmt:PES") return "Pour ma Brother (PES)";
  if (view === "fmt:AUTRES") return "Autres formats";
  if (view.startsWith("tag:")) return "« " + view.slice(4) + " »";
  if (view === "dir:") return "Ajoutées une par une";
  if (view.startsWith("dir:")) return view.slice(4);
  return "Toutes mes broderies";
}
