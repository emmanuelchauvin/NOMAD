# Reprise du projet sur une autre machine

État au 18 novembre 2026 au soir, branche `weibull-benchmark`. Cette fiche sert à reprendre
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
| [RESULTATS_GLM_H3.md](RESULTATS_GLM_H3.md) | glm-5.3-flash, évolution avec élagage | 1 359 appels, 24 seeds | **H1 (24 seeds sur 24), H2 établies**, H3 non (p = 0,60) |

H1 : à appels égaux, l'évolution bat le tirage. H2 : à dépense égale aussi. H3 : l'avance
est plus grande à 1 359 appels qu'à 340.

## Où on en est le 18 novembre

Deux machines : le **laptop** (sans GPU) fait tourner les tirs ; la **tour** (RTX) sert aux
essais et aux bancs hors ligne. Une branche par machine pour éviter les conflits git.

**Branches**

- `weibull-benchmark` : protocoles, résultats, scripts de campagne et d'analyse.
- `ast-croisement` (worktree `../nomad-ast` sur le laptop) : tout le code récent, y compris
  le travail de la tour du 16 au 17 novembre (branche `tour-alignement-c`, fusionnée en
  avance rapide) : groupe C aligné sur le groupe D, greffe corrigée (variables du donneur
  renommées, réaffectations dans la tranche, vivacité sans `score` codé en dur), mode de
  greffe `RulesOnly`. 97 tests passent. Le tag `glm-h2-elagage` (`7caeeb1`) reste
  l'exécutable figé des protocoles H2 et H3.

**Fait du 16 au 18 novembre**

- **H2 sur GLM avec l'élagage, 344 appels : H1 et H2 établies**
  ([RESULTATS_GLM_H2.md](RESULTATS_GLM_H2.md)), 2,03 $.
- **H3 sur GLM, 1 359 appels : H1 et H2 établies, H3 non** (p = 0,60)
  ([RESULTATS_GLM_H3.md](RESULTATS_GLM_H3.md)), 11,90 $. L'évolution gagne les 24 seeds à
  1 359 appels, mais son avance médiane ne grandit pas (22,5 points à 344 appels, 25,9 à
  1 359) : sur GLM, le tirage progresse encore, contrairement à Gemini.
- **Groupe C aligné sur le groupe D**, greffe corrigée (tour).
- **Banc mélanges contre règles complètes** (tour, `docs/MESURE_GREFFE_MELANGE_REGLES.md`
  sur `ast-croisement`) : autant d'enfants gardés dans les deux modes, gains plus grands et
  plus solides en validation avec les règles complètes (80 % contre 68 %), code ajouté cinq
  fois plus gros. Exploratoire, sans test fixé à l'avance.

**À faire ensuite**

1. **Corriger l'ordre de `BenchmarkScore`** (gratuit, avant le prochain exécutable figé) :
   `tldr_score` y est comparé avec une tolérance de 1e-4, ce qui ne donne pas un ordre
   total. Un run de tirage du protocole H3 s'est arrêté dessus en fin de run (relancé
   depuis le début). Cause probable, à reproduire par un test avant de corriger.
2. **Brancher le mode de greffe dans le moteur** (gratuit, tour) : `RulesOnly` n'existe
   aujourd'hui que dans le banc. Variable d'environnement, élagage du champion aussi dans
   le groupe C, tests.
3. **Limiter la taille du code** : sur GLM, le champion final de l'évolution fait 9 120
   caractères à 1 359 appels et le coût d'un appel triple au fil du run. Pistes : greffe
   qui remplace au lieu d'empiler (revue d'Argon), élagage plus fort. À mesurer hors ligne
   avant tout protocole.
4. **Protocole groupe C contre groupe D sur GLM** : groupe C aligné, règles complètes et
   élagage, contre groupe D avec élagage. Environ 3 $, avec un nouvel exécutable figé
   (points 1 et 2).
5. **Commande CLI des îles**, puis protocole îles contre une seule population.
6. **Ordonnancement** : benchmark peu discriminant, à refaire avant tout protocole. Sur la
   seed 9600, First-Fit est à 37 nœuds de L1 sur 75 instances, et les quatre heuristiques
   classiques tiennent en 5 nœuds.

**À décider pour les prochains protocoles** : définir le point de dépense égale par la
dépense cumulée plutôt que par le coût moyen par appel (voir RESULTATS_GLM_H3.md).

**Acquis, pour mémoire**

- Relances : elles fondent la lignée du champion final dans 7 runs GLM sur 24 et 11
  Gemini sur 24 ; ne pas les réduire.
- Aiguillage de régime : non retenu (gains ≤ 0,2 %, fuel doublé).
- Gabarits de repli : 2,4 % des propositions GLM, 0,1 % Gemini, aucun champion à un
  palier ; résultats publiés non touchés (RESULTATS_GLM_340.md).
- Élagage : ~3 fois moins de code, coût par appel divisé par 2, pas de gain de qualité à
  appels égaux (RESULTATS_GLM_LOCAL.md). Il n'arrête pas le grossissement sur un run long
  (RESULTATS_GLM_H3.md).
- Effet d'échelle : établi sur Gemini, où le tirage plafonne ; pas sur GLM, où le tirage
  progresse encore à 1 359 appels (RESULTATS_GLM_H3.md).
- Point de dépense égale `k* = ⌊budget / r⌋`, avec `r` moyen sur le run : défavorable à
  l'évolution, dont les premiers appels coûtent moins (GLM, 1 359 appels : à `k*` = 393,
  66 % de la dépense du tirage complet).

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
  `glm-local-v2` (SHA-256 de l'original `460636844eb4…9044`), celui des protocoles H2 et
  H3 sur GLM (`nomad-h2-elagage.exe`) depuis le tag `glm-h2-elagage` (SHA-256 de
  l'original `9e6e3098f077…dae99b`).
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
  20,65 $ dépensés au 18 novembre, **recharge automatique activée**). Les deux
  machines partagent le même compte : ne pas lancer deux tirs payants en même temps sans
  le savoir.
  Les clés Gemini et MiniMax sont aussi enregistrées dans OpenRouter : `gemini-3.1-flash-lite`
  et MiniMax M3 y sont utilisables (vérifier la facturation avec une clé propre).
