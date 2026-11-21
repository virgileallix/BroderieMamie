export const $ = id => document.getElementById(id);

/** Minuscules, sans accents : « Noël » → « noel ». */
export const norm = s => (s || "").toLowerCase().replace(/œ/g, "oe").replace(/æ/g, "ae").normalize("NFD").replace(/[̀-ͯ]/g, "");

export const plural = (n, one, many) => `${n.toLocaleString("fr-FR")} ${n > 1 ? many ?? one + "s" : one}`;

export const cm = mm => (mm / 10).toLocaleString("fr-FR", { maximumFractionDigits: 1 });
export const size = (w, h) => `${cm(w)} × ${cm(h)} cm`;

export function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k === "class") e.className = v;
    else if (k === "style") e.style.cssText = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children.flat()) if (c != null && c !== false) e.append(c.nodeType ? c : String(c));
  return e;
}

export function icon(name, cls = "icon") {
  const ns = "http://www.w3.org/2000/svg";
  const s = document.createElementNS(ns, "svg");
  s.setAttribute("class", cls);
  s.setAttribute("aria-hidden", "true");
  const u = document.createElementNS(ns, "use");
  u.setAttribute("href", "#i-" + name);
  s.append(u);
  return s;
}

export function joinPath(dir, file) {
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + file : dir + sep + file;
}

export const prefs = {
  get(k, d) { try { return localStorage.getItem("bm." + k) ?? d; } catch { return d; } },
  set(k, v) { try { localStorage.setItem("bm." + k, v); } catch { /* rien */ } },
};
