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

/** Les broderies à afficher, filtrées par la vue et la recherche, puis triées. */
export function visible() {
  const words = norm(state.query).split(/\s+/).filter(Boolean);
  const list = state.entries.filter(e => {
    if (!inView(e)) return false;
    if (!words.length) return true;
    const hay = norm([e.name, e.originalName, e.label, e.tags.join(" "), e.ext, e.format,
      e.colors.map(c => c.name).join(" ")].join(" "));
    return words.every(w => hay.includes(w));
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
  return "Toutes mes broderies";
}
