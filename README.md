# 🧵 Les broderies de Mamie

Logiciel Windows pour ranger, retrouver et voir ses motifs de broderie machine :
**.pes** (Brother), .pec, **.jef** et .sew (Janome), **.vp3**/.pv3 et **.vip** (Pfaff), **.hus** (Husqvarna), .exp (Melco), .dst (Tajima).

- **Ajouter** des fichiers, **plusieurs dossiers d'un coup** ou des **.zip** (boutons, ou glisser-déposer depuis l'Explorateur).
  Chaque dossier ou .zip ajouté devient un **dossier de la bibliothèque, à son nom** (rubrique « Mes dossiers »,
  et sur le disque `Motifs/<nom>/`).
- **Doublons** : un fichier strictement identique (même contenu, mêmes points) n'est pas ajouté deux fois.
  Le **même motif** dans un autre format ou une autre version (même taille, même dessin) est repéré, et
  **« Chercher les doublons »** les montre côte à côte : on choisit ce qu'on garde (le PES est conseillé, la
  machine est une Brother). Les mots-clés et étoiles passent à la broderie gardée ; une paire gardée exprès
  n'est plus reproposée.

**Rien n'est jamais effacé de l'ordinateur** : les fichiers d'origine ne sont que copiés dans la bibliothèque,
jamais modifiés ni supprimés. « Supprimer » range la copie dans le dossier `Corbeille` de la bibliothèque.
- **Rechercher** par nom, mot-clé ou couleur (sans se soucier des accents) ; favoris ; mots-clés dans le menu.
- **Voir** chaque motif en grand avec ses vraies couleurs, zoom, fond lin/blanc/noir, cadre conseillé,
  et l'animation **« Voir la broderie se faire »** point par point.
- **Copier sur la clé USB** pour la machine, **Tout sauvegarder** dans un .zip.

Les broderies sont rangées dans **Documents › Broderies de Mamie** :

```
Broderies de Mamie/
├── Motifs/            ← les fichiers, bien nommés, rangés par dossier importé (Motifs/Noël/…)
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
