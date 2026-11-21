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
    dedupe: async () => { await wait(600); return window.__MOCK_DEDUPE__ || { removed: [], kept: entries.length }; },
    pickFiles: async () => ["C:/Users/Mamie/Téléchargements/motifs.zip"],
    pickFolder: async () => ["E:/"],
    saveZip: async p => "C:/" + p,
    thumbUrl: p => encodeURI(p), // servi par un petit serveur http local (voir dev/README.md)
    reveal: async () => {},
    onProgress: cb => { progress = cb; },
    onDragDrop: () => {},
  };
})();
