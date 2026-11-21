# 🧵 Les broderies de Mamie

Logiciel Windows pour ranger, retrouver et voir ses motifs de broderie machine (**.pes**, **.vp3**, **.pv3**, .pec).

- **Ajouter** des fichiers, un dossier entier ou un **.zip** (bouton, ou glisser-déposer depuis l'Explorateur).
- **Doublons supprimés automatiquement** : même fichier sous un autre nom, ou même dessin enregistré dans
  une autre version — un compte rendu dit ce qui a été ajouté et retiré.
- **Rechercher** par nom, mot-clé ou couleur (sans se soucier des accents) ; favoris ; mots-clés dans le menu.
- **Voir** chaque motif en grand avec ses vraies couleurs, zoom, fond lin/blanc/noir, cadre conseillé,
  et l'animation **« Voir la broderie se faire »** point par point.
- **Copier sur la clé USB** pour la machine, **Tout sauvegarder** dans un .zip.

Les broderies sont rangées dans **Documents › Broderies de Mamie** :

```
Broderies de Mamie/
├── Motifs/            ← les fichiers, bien nommés (on peut aussi les ouvrir avec un autre logiciel)
├── Corbeille/         ← les broderies supprimées (rien n'est perdu)
├── Aperçus/           ← les images (refaites automatiquement)
└── bibliotheque.json  ← noms, mots-clés, favoris
```

## Installer

Sur GitHub, page **Releases** → `Les.broderies.de.Mamie_x.y.z_x64-setup.exe`. Sinon, onglet **Actions** → dernier « Logiciel Windows (.exe) » réussi → **Installeur-Broderies-de-Mamie**.
Dans le .zip, double-cliquer sur le fichier qui finit par `setup.exe`.
Plus simple : la page **Releases** du dépôt contient toujours le dernier installeur (publié automatiquement à chaque mise à jour de `main`).

## Mises à jour automatiques

Au démarrage, le logiciel regarde s'il y a une nouvelle version sur la page Releases ; si oui, il propose
« Oui, mettre à jour » et s'installe tout seul (les broderies ne bougent pas).

**Sortir une nouvelle version :** changer `"version"` dans `src-tauri/tauri.conf.json` (ex. `1.2.0`),
puis pousser sur `main`. GitHub fabrique l'installeur, le signe et publie la Release `v1.2.0`.

Les mises à jour sont signées : les secrets `TAURI_SIGNING_PRIVATE_KEY` et `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
doivent exister dans *Settings › Secrets and variables › Actions*. Sans eux, l'installeur est fabriqué
mais sans mise à jour automatique (un avertissement apparaît dans Actions). La clé publique correspondante
est dans `src-tauri/tauri.conf.json` — si on perd la clé privée, il faut en refaire une et réinstaller une fois à la main.

## Organisation du code

| Dossier | Rôle |
|---|---|
| `crates/broderie-core/` | **Rust** — lecture PES/PEC/VP3, empreintes anti-doublons, aperçus PNG, bibliothèque sur disque. Testé (`cargo test -p broderie-core`). |
| `src-tauri/` | **Rust** — application de bureau [Tauri 2] : fenêtre, commandes appelées par l'interface. |
| `ui/` | Interface : `index.html`, `css/` (jetons, composants, mise en page, fiche) et `js/` (modules ES). |
| `dev/` | Faux pont pour tester l'interface dans un navigateur. |
| `.github/workflows/` | Fabrication automatique de l'installeur Windows. |

Design : maquettes Figma « Broderies de Mamie — Maquettes » (palette « lin & fil », Fraunces + Nunito).

## Développer

```bash
npm install
npm run dev      # lance l'application (Windows, macOS ou Linux avec webkit2gtk)
npm run build    # fabrique l'installeur
cargo test -p broderie-core
```

[Tauri 2]: https://tauri.app
