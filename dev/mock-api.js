// Faux pont pour essayer l'interface dans un navigateur, sans l'application.
// Les données viennent de `window.__MOCK_DATA__` (produites par l'exemple `demo` du cœur Rust).
(() => {
  const data = window.__MOCK_DATA__;
  let entries = structuredClone(data.entries);
  const wait = ms => new Promise(r => setTimeout(r, ms));
  let progress = () => {};
  window.__MOCK_API__ = {
    info: async () => ({ root: data.root, thumbs: data.thumbs, entries: structuredClone(entries) }),
    importPaths: async () => {
      const total = 12;
      for (let i = 0; i <= total; i++) { progress({ done: i, total, file: `motif_${i + 1}.pes` }); await wait(60); }
      return window.__MOCK_IMPORT__ || data.report;
    },
    update: async (id, up) => {
      const e = entries.find(x => x.id === id);
      if (up.name) e.name = up.name;
      if (up.tags) e.tags = up.tags;
      if (up.favorite != null) e.favorite = up.favorite;
      return structuredClone(e);
    },
    remove: async id => { entries = entries.filter(e => e.id !== id); },
    design: async id => data.designs[id],
    exportTo: async () => "E:/broderie.pes",
    backup: async () => entries.length,
    findDuplicates: async () => { await wait(300); return (window.__MOCK_GROUPS__ || (data.groups || []).map(g => g.ids)).map(g => ({ ids: g.filter(id => entries.some(e => e.id === id)) })).filter(g => g.ids.length > 1); },
    removeDuplicates: async choices => { let n = 0; for (const c of choices) for (const id of c.remove) { entries = entries.filter(e => e.id !== id); n++; } window.__MOCK_GROUPS__ = []; return n; },
    pickFiles: async () => ["C:/Users/Mamie/Téléchargements/motifs.zip"],
    pickFolder: async () => ["E:/"],
    pickFolders: async () => ["C:/Users/Mamie/Broderies/Noël", "C:/Users/Mamie/Broderies/Fleurs"],
    saveZip: async p => "C:/" + p,
    thumbUrl: p => encodeURI(p), // servi par un petit serveur http local (voir dev/README.md)
    reveal: async () => {},
    aiStatus: async () => window.__MOCK_AI__ || { hasKey: false, running: false, analysed: 0, total: entries.length, error: null },
    aiSetKey: async key => { await wait(500); if (key.length < 20) throw "Cette clé semble incomplète."; window.__MOCK_AI__ = { hasKey: true, running: true, analysed: 3, total: entries.length, error: null }; return window.__MOCK_AI__; },
    aiRemoveKey: async () => (window.__MOCK_AI__ = { hasKey: false, running: false, analysed: 3, total: entries.length, error: null }),
    aiStart: async () => window.__MOCK_AI__,
    onAiProgress: () => {},
    openUrl: async () => {},
    entryPath: async id => "/lib/Motifs/" + entries.find(e => e.id === id).fileName,
    onProgress: cb => { progress = cb; },
    onDragDrop: () => {},
  };
})();
