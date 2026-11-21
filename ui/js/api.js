// Pont vers la partie Rust (Tauri). En développement dans un simple navigateur,
// un faux pont peut être fourni par `window.__MOCK_API__` (voir dev/mock-api.js).

const T = window.__TAURI__;

function tauriApi() {
  const invoke = (cmd, args) => T.core.invoke(cmd, args);
  const EXT = ["pes", "pec", "vp3", "pv3", "jef", "sew", "exp", "dst", "hus", "vip", "zip"];
  const asList = v => (v == null ? [] : Array.isArray(v) ? v : [v]);
  return {
    info: () => invoke("library_info"),
    importPaths: paths => invoke("import_paths", { paths }),
    update: (id, update) => invoke("update_entry", { id, update }),
    remove: id => invoke("delete_entry", { id }),
    design: id => invoke("get_design", { id }),
    exportTo: (id, dir) => invoke("export_entry", { id, dir }),
    backup: dest => invoke("backup", { dest }),
    findDuplicates: () => invoke("find_duplicates"),
    removeDuplicates: choices => invoke("remove_duplicates", { choices }),
    pickFiles: async () => asList(await T.dialog.open({
      multiple: true, title: "Choisir des broderies",
      filters: [{ name: "Broderies (.pes, .jef, .vp3, .hus…, .zip)", extensions: EXT }],
    })),
    pickFolder: async title => asList(await T.dialog.open({ directory: true, multiple: false, title })),
    pickFolders: async title => asList(await T.dialog.open({ directory: true, multiple: true, title })),
    saveZip: defaultPath => T.dialog.save({ title: "Enregistrer la sauvegarde", defaultPath, filters: [{ name: "Fichier zip", extensions: ["zip"] }] }),
    thumbUrl: path => T.core.convertFileSrc(path),
    reveal: path => T.opener.revealItemInDir(path),
    onProgress: cb => T.event.listen("import-progress", e => cb(e.payload)),
    onDragDrop: cb => T.webview.getCurrentWebview().onDragDropEvent(e => cb(e.payload)),
  };
}

export const api = T ? tauriApi() : window.__MOCK_API__;
export const available = Boolean(api);
