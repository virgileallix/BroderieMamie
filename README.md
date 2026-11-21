# 🧵 Les broderies de Mamie

Un petit logiciel pour ranger les motifs de broderie machine (**.pes**, **.vp3**, **.pv3**, .pec).

## Ce qu'il fait

- **Ajouter des broderies** : bouton, dossier entier, glisser-déposer, ou même un fichier **.zip** téléchargé sur internet.
- **Supprime les doublons automatiquement** : un fichier déjà présent (même contenu, même s'il porte un autre nom)
  ou un motif identique enregistré dans une autre version n'est pas ajouté une deuxième fois. Un rapport dit ce qui a été retiré.
- **Rechercher** par nom ou par mots-clés (« fleur », « Noël »…), sans se soucier des accents ni des majuscules.
  Filtres PES / VP3 / Favoris, tri par nom, date ou taille.
- **Visualiser** chaque motif avec ses vraies couleurs de fil : taille en cm, nombre de points, couleurs,
  cadre conseillé, zoom, fond blanc / tissu / noir, et l'animation **« Voir la broderie se faire »** point par point.
- **Télécharger** un motif pour le mettre sur la clé USB de la machine.
- **Tout sauvegarder** en un seul fichier .zip (à garder sur une clé USB). Pour restaurer : glisser ce .zip dans le logiciel.

## Comment l'utiliser

1. Copier le fichier `index.html` sur l'ordinateur (par exemple sur le Bureau, renommé « Broderies de Mamie.html »).
2. Double-cliquer dessus : il s'ouvre dans le navigateur (**Microsoft Edge**, **Google Chrome** ou Firefox).
3. C'est tout ! Pas d'installation, pas besoin d'internet.

> ⚠️ Les broderies sont gardées **dans le navigateur de cet ordinateur**. Il faut toujours ouvrir le fichier
> avec le même navigateur, et ne pas effacer les « données de navigation / cookies et données de site ».
> Faire régulièrement « Tout sauvegarder » est une bonne habitude.

## Remarques techniques

- Tout est dans un seul fichier `index.html` (HTML + JavaScript, aucune dépendance).
- Les fichiers `.pv3` sont lus comme des `.vp3` (le format est reconnu d'après le contenu du fichier).
- Les PES version 5 et plus donnent les couleurs exactes ; les plus anciens utilisent la palette Brother standard.
- Un même motif enregistré en .pes **et** en .vp3 est gardé en double exprès : ce sont deux fichiers pour deux machines différentes.
