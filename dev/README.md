# Essayer l'interface sans l'application

L'interface (`ui/`) parle normalement à la partie Rust via Tauri. Pour la tester dans un navigateur :

1. Créer une bibliothèque de démonstration et exporter ses données :
   `cargo run -p broderie-core --example demo -- <dossier de .pes/.vp3> <dossier bibliothèque> > mock.json`
2. Servir `ui/` et le dossier bibliothèque avec un petit serveur http local
   (les modules JavaScript ne se chargent pas en `file://`).
3. Avant le chargement de la page, définir `window.__MOCK_DATA__` (contenu de `mock.json`, avec `thumbs`
   pointant vers l'URL des aperçus) puis charger `dev/mock-api.js`.
