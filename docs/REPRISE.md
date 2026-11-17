# Reprise du projet sur une autre machine

État au 17 novembre 2026 au matin, branche `weibull-benchmark`. Cette fiche sert à reprendre
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
| [RESULTATS_GLM_LOCAL.md](RESULTATS_GLM_LOCAL.md) | glm-5.3-flash, évolution avec / sans croisement local et élagage | 344 appels, 12 seeds | H non établie (p = 0,77) ; coût par appel divisé par 2 |
| [RESULTATS_GLM_H2.md](RESULTATS_GLM_H2.md) | glm-5.3-flash, évolution avec élagage | 344 appels, 24 seeds | **H1, H2 établies** (r = 2,12, k* = 162, p = 0,014) |

H1 : à appels égaux, l'évolution bat le tirage. H2 : à dépense égale aussi. H3 : l'avance
est plus grande à 1 359 appels qu'à 340.

## Où on en est le soir du 16 novembre

Deux machines : le **laptop** (sans GPU) fait tourner les tirs ; la **tour** (RTX) sert aux
essais et aux bancs hors ligne. Une branche par machine pour éviter les conflits git.

**Branches**

- `weibull-benchmark` : protocoles, résultats, scripts de campagne et d'analyse.
- `ast-croisement` (worktree `../nomad-ast` sur le laptop) : tout le code récent.
  Croisement AST corrigé, opérateurs locaux du groupe D (`NOMAD_LOCAL_CROSSOVERS`,
  `NOMAD_PRUNE_CHAMPION`), îles (`export_migrants` / `import_migrants`), étiquette
  `fallback_template`, corrections d'Argon. 87 tests passent. Mesures hors ligne dans
  `mesures/` et `docs/MESURE_AIGUILLAGE.md`.

**Fait dans la nuit du 16 au 17 novembre**

1. **H2 sur GLM avec l'élagage.** Bras évolution + élagage (sans croisement local) contre
   tirage, 24 seeds neuves, 344 appels, même exécutable figé pour les deux bras ; H1 à
   appels égaux, H2 à dépense égale avec `k* = 344 / r`, `r` mesuré. Coût estimé ~2,10 $ ;
   si l'essai de fumée projette plus de 3 $, demander avant de lancer. Avant de figer
   l'exécutable : rendre **invalides** les propositions remplacées par un gabarit de
   repli (aujourd'hui seulement étiquetées). Durée estimée 8 à 12 h, à lancer sur le
   laptop. **Fait : H1 et H2 établies** ([RESULTATS_GLM_H2.md](RESULTATS_GLM_H2.md)), 2,03 $.

**À faire ensuite, gratuit**

2. **Aligner le groupe C sur le groupe D** avant tout protocole C contre D : mutation
   informée, relances, enfant accepté seulement s'il bat son hôte, suppression de l'appel
   au modèle de secours dans `HttpLlamaOperator::crossover` (son enfant, sans manifeste,
   est rejeté à 100 % par le groupe C).
3. **Greffe** : renommer les variables du donneur (`_d_v`) et inclure ses réassignations
   dans la tranche ; `compute_liveness` sans `score` codé en dur, avec
   `collect_free_variables`.
4. **Banc mélanges contre règles complètes** (bon candidat pour la tour, plusieurs heures
   de CPU). Argon propose de supprimer les mélanges `score * 0,95 + 0,05 × variable du
   donneur` du croisement. Mais dans le tir GLM local, 845 croisements sur 1 656 en
   ajoutent un, et 25 des 45 champions issus d'un croisement viennent d'un mélange ajouté.
   À comparer hors ligne, sur les mêmes paires, à une greffe de règles complètes avant de
   décider. Point d'Argon exact mais sans effet dans NOMAD : `let score = …` sans `mut`
   ne compilerait pas en Rust, notre interpréteur l'accepte.
5. **Commande CLI des îles**, puis protocole.
6. **H3 sur GLM** si H2 est établie.
7. **Ordonnancement** : benchmark peu discriminant, à refaire avant tout protocole. Sur la
   seed 9600, First-Fit est à 37 nœuds de L1 sur 75 instances, et les quatre heuristiques
   classiques tiennent en 5 nœuds.

**Acquis, pour mémoire**

- Relances : elles fondent la lignée du champion final dans 7 runs GLM sur 24 et 11
  Gemini sur 24 ; ne pas les réduire.
- Aiguillage de régime : non retenu (gains ≤ 0,2 %, fuel doublé).
- Gabarits de repli : 2,4 % des propositions GLM, 0,1 % Gemini, aucun champion à un
  palier ; résultats publiés non touchés (RESULTATS_GLM_340.md).
- Élagage : ~3 fois moins de code, coût par appel divisé par 2, pas de gain de qualité à
  appels égaux (RESULTATS_GLM_LOCAL.md).

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
- Pour le code récent : `git worktree add ../nomad-ast ast-croisement`, puis `cargo test`
  dans ce dossier. L'exécutable du protocole GLM local se reconstruit depuis le tag
  `glm-local-v2` (SHA-256 de l'original `460636844eb4…9044`).
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
  automatique), OpenRouter (environ
  8,75 $ dépensés au 17 novembre au matin, **recharge automatique activée**). Les deux
  machines partagent le même compte : ne pas lancer deux tirs payants en même temps sans
  le savoir.
  Les clés Gemini et MiniMax sont aussi enregistrées dans OpenRouter : `gemini-3.1-flash-lite`
  et MiniMax M3 y sont utilisables (vérifier la facturation avec une clé propre).
