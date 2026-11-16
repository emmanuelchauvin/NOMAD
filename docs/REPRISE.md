# Reprise du projet sur une autre machine

État au 16 novembre 2026, branche `weibull-benchmark`. Cette fiche sert à reprendre
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
| [RESULTATS_GLM_340.md](RESULTATS_GLM_340.md) | glm-5.3-flash (OpenRouter) | 344 appels, 24 seeds | **H1 établie**, H2 non (coût par appel ×4,4) |

H1 : à appels égaux, l'évolution bat le tirage. H2 : à dépense égale aussi. H3 : l'avance
est plus grande à 1 359 appels qu'à 340.

## Prochaines étapes, dans l'ordre

1. **Croisement AST** (worktree `../nomad-ast`, branche `ast-croisement`). Corrigé les 15 et
   16 novembre : sondes du niveau 3 sans OOD, enfant rejeté jamais champion, analyse de
   dépendances par portée, greffe non résolue refusée, locus 1 du croisement multi-locus
   placé après la déclaration de `score` (48 enfants sur 192 étaient invalides à cause de
   lui). Banc hors ligne (`mesures/`, 12 runs Gemini, 192 enfants) : 6 enfants battent le
   champion de leur run, tous confirmés en validation (jusqu'à −80 bacs), dont 4 entre
   îles. Restent à corriger : appel au modèle caché dans le croisement avant le contrôle
   du budget, contrôle de niveau 1 vide, archive non déterministe (HashMap), niveau 3 qui
   accepte un enfant moins bon que son hôte, anciens usages de `collect_dependencies`.
   Puis protocole groupe C contre groupe D.
2. **Élagage par ablation** (`examples/prune_champions.rs`) : sur les 24 champions Gemini,
   45 règles sur 273 retirées sans perte, 130 bacs gagnés en validation (seed 8136 :
   −84). À brancher dans le moteur, aussi pour réduire le coût par appel, cause de l'échec
   de H2 sur GLM.
3. **Relances** : elles ne produisent presque aucun champion (Gemini et GLM). Mesurer si
   leurs descendants en produisent, puis tester un moteur avec moins de relances.
4. **H3 sur un second modèle** (GLM, MiniMax M3 ou DeepSeek par OpenRouter). Essai
   préalable court d'abord.
5. **Autre problème** : l'ordonnancement, après vérification que son benchmark est sain.
6. **Îles** : export/import d'archives entre populations sur une machine, puis protocole.

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
- Crédits : Gemini (clé Google, 17,23 € restants au 15 novembre, sans recharge
  automatique), OpenRouter (12 $ achetés, environ
  4,25 $ dépensés au 16 novembre après le tir GLM à 344 appels, sans recharge automatique).
  Les clés Gemini et MiniMax sont aussi enregistrées dans OpenRouter : `gemini-3.1-flash-lite`
  et MiniMax M3 y sont utilisables (vérifier la facturation avec une clé propre).
