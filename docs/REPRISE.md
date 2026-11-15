# Reprise du projet sur une autre machine

État au 15 novembre 2026, 15 h 30, branche `weibull-benchmark`. Cette fiche sert à reprendre
le travail sur un autre PC ou avec un autre assistant : elle résume où en est la preuve et ce
qui reste à faire.

## Objectif

Montrer, avec des protocoles fixés à l'avance, que l'évolution de NOMAD fait mieux qu'un
tirage de propositions indépendantes du même modèle, à budget égal, et que l'écart grandit
avec le budget. C'est le préalable à la grille évolutive distribuée (evoluthink.com).

## Méthode de travail

1. Écrire le protocole dans `docs/PROTOCOLE_*.md` et le commiter **avant** tout run :
   question, bras, seeds, mesures, tests et seuils.
2. Essai préalable sur des seeds hors protocole : coût, durée, validité, rien d'autre.
3. Aucun tir payant sans le go explicite de l'utilisateur.
4. Commiter les données brutes avant de les lire, puis le script de dépouillement en mode
   `controle` (aucun score affiché).
5. Dépouiller avec l'utilisateur, puis écrire `docs/RESULTATS_*.md`.

## Où en est la preuve

| Protocole | Modèle | Budget | Résultat |
|---|---|---|---|
| [RESULTATS_GEMINI_CONFIRMATION.md](RESULTATS_GEMINI_CONFIRMATION.md) | gemini-3.1-flash-lite | 85 appels | H1 établie |
| [RESULTATS_GEMINI_ECHELLE.md](RESULTATS_GEMINI_ECHELLE.md) | gemini-3.1-flash-lite | 340 appels | H1, H2 établies |
| [RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md) | gemini-3.1-flash-lite | 1 359 appels, 24 seeds | **H1, H2, H3 établies** |
| [PROTOCOLE_GLM_COURBE.md](PROTOCOLE_GLM_COURBE.md) | glm-5.3-flash (OpenRouter) | 1 359 appels | abandonné après l'essai préalable |

H1 : à appels égaux, l'évolution bat le tirage. H2 : à dépense égale aussi. H3 : l'avance
est plus grande à 1 359 appels qu'à 340.

## Prochaines étapes, dans l'ordre

1. **Réplication sur GLM à 340 appels.** Nouveau protocole `docs/PROTOCOLE_GLM_340.md`
   calqué sur [PROTOCOLE_GLM_COURBE.md](PROTOCOLE_GLM_COURBE.md) : mêmes réglages, 24 seeds
   neuves (par exemple 9000 à 9391), évolution `-n 47` avec `NOMAD_INIT_POP=22`
   (22 + 46 × 7 = 344 : à ajuster pour tomber sur 340, ou garder les paliers sur les
   340 premières propositions du journal), tirage `--pop-size 340 -n 1`. Tests H1 et H2,
   seuil 0,05. Coût estimé : environ 2,50 $. Durée : une nuit sur l'ancien PC.
2. **H3 sur un autre modèle**, qui ne fasse pas grossir les programmes à la mutation
   (candidat : `deepseek/deepseek-v4.1-flash` sur OpenRouter, environ 6 $ pour 12 seeds).
   Essai préalable court d'abord.
3. **Croisement AST** : le corriger et l'ajouter au moteur du groupe D derrière une option,
   puis le tester contre l'évolution seule. Deux défauts du groupe C à corriger d'abord,
   notés dans [RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md).
4. **Autre problème** : l'ordonnancement, après vérification que son benchmark est sain.
5. **Îles** : export/import d'archives entre populations sur une machine, puis protocole.

## Installer l'environnement

- Rust (testé avec 1.93.1) et Python 3.10 ou plus récent (bibliothèque standard seulement).
- Cloner le dépôt et se placer sur la branche `weibull-benchmark`.
- **Reconstruire l'exécutable figé**, qui n'est pas dans git (`target/` est ignoré) :

  ```
  git worktree add ../nomad-gemini-courbe gemini-courbe
  cd ../nomad-gemini-courbe
  cargo build --release -p nomad-cli
  mkdir ..\NOMAD\target\frozen
  copy target\release\nomad.exe ..\NOMAD\target\frozen\nomad-gemini-courbe.exe
  ```

  Tous les protocoles à partir de `gemini-courbe` utilisent cet exécutable. Une version
  recompilée sur une autre machine n'aura pas forcément la même empreinte que l'original
  (SHA-256 `1f16312c10511e5407bf316a1aa12be32f05d6f4549dc35e946f9dbe66ce4c22`) : c'est le
  même code source, et le contrôle ci-dessous suffit à vérifier qu'il se comporte pareil.
- Vérifier : `cargo test` sur la branche, puis
  `python scripts/analysis/gemini_courbe.py controle --seeds24` doit afficher
  « 0 probleme(s) ».
- Clés d'API, en variables d'environnement **utilisateur** Windows (jamais dans un
  fichier) : `GEMINI_API_KEY` et `OPENROUTER_API_KEY`. Les scripts de campagne les lisent au
  niveau utilisateur.

## Particularités connues

- Un antivirus qui inspecte le HTTPS (Avast sur l'ancien PC) fait échouer les certificats :
  l'exécutable utilise les certificats de Windows (`ureq` avec `native-certs`), et pour git,
  `git -c http.sslBackend=schannel push`.
- Les tirs sont lancés détachés par `scripts/campaign/lancer_*.ps1` ; `-Etat` affiche
  l'avancement. Relancer un lanceur reprend les seeds non terminées.
- Les tirs utilisent le processeur pour évaluer les programmes, pas la carte graphique.
  Douze processus en parallèle chargent fortement un processeur à 6 cœurs.
- Crédits : Gemini (clé Google, recharge automatique), OpenRouter (12 $ achetés, environ
  0,75 $ dépensés au 15 novembre, sans recharge automatique).
