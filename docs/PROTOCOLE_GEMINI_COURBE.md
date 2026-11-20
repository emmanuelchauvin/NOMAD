# Protocole de courbe d'échelle sur Gemini : tirage contre évolution jusqu'à 1 359 appels (fixé avant exécution)

Rédigé le 14 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

L'avance de l'évolution sur le tirage grandit-elle quand le budget augmente ?

Le protocole précédent ([RESULTATS_GEMINI_ECHELLE.md](RESULTATS_GEMINI_ECHELLE.md)) ne l'a
pas montré, avec un test peu sensible : il rapprochait deux expériences faites sur des
seeds différentes. Celui-ci mesure l'avance à plusieurs budgets **sur les mêmes seeds, dans
les mêmes runs**.

## Principe

Depuis le commit `cd6d18c`, chaque run enregistre toutes ses propositions dans l'ordre,
et note sur la validation chacun de ses champions successifs. Un run de 1 359 appels donne
donc aussi le champion qu'il avait après 85, 170, 340 ou 680 appels.

Un run long arrêté en route est un run court : le moteur ne règle rien sur la longueur
prévue du run (vérifié par le test
`test_a_long_run_cut_short_is_the_short_run`). Pour le tirage, les `k` premières
propositions sont un tirage de `k` propositions.

## Dispositif commun aux deux bras

- Benchmark, modèle et réglages du modèle : ceux des deux protocoles précédents, inchangés
  (profil Weibull, fuel 400 000, `gemini-3.1-flash-lite`, raisonnement au minimum,
  température 0,7, top_p 0,95, sans champ `seed`).
- 12 seeds neuves : 8000, 8017, ..., 8187. Aucune n'a servi à un essai.
- Code figé par le tag git `gemini-courbe`, exécutable
  `target/frozen/nomad-gemini-courbe.exe`. Différences avec le tag `gemini-echelle` : le
  journal des propositions (passif), et le client HTTP qui rejoue une réponse coupée ou
  vide (six essais, attentes de 4 à 180 secondes). La sélection, la mutation,
  l'interpréteur et les prompts sont inchangés.
- Lancement par `scripts/campaign/run_gemini_courbe_arm.ps1`, les deux bras en parallèle,
  chacun réparti sur six processus. Les runs sont indépendants.

## Les deux bras

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 1 359 propositions fraîches | `run -G D --pop-size 1359 -n 1` |
| Évolution | Groupe D, 22 propositions fraîches puis 191 générations de 7 appels (1 359 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée | `run -G D --pop-size 8 -n 192`, `NOMAD_INIT_POP=22` |

Le bras d'évolution est exactement celui du protocole de confirmation (départ de 22
propositions, établi à 85 appels), prolongé. Le départ de 88 propositions du protocole
d'échelle n'est pas repris : voir [RESULTATS_COURBE_ECHELLE.md](RESULTATS_COURBE_ECHELLE.md),
observation 4.

**Essai préalable.** Deux runs d'évolution de 680 appels sur des seeds hors protocole
(3051 et 3068 ; données dans `results_gemini_courbe_pilote/`). Seuls ont été regardés : la
cohérence du journal, les tokens, le coût et la durée. Coût : 1,04 $ et 1,09 $, soit
environ 0,0016 $ par appel, sans dérive nette entre le début et la fin du run. Coût prévu
du protocole : environ 26 $ pour l'évolution et 17 $ pour le tirage.

## Mesures et lecture, fixées à l'avance

**Paliers.** 85, 170, 340, 680 et 1 359 appels.

**Grandeur mesurée.** Pour une seed, un bras et un palier `k` : l'effet sur la validation du
champion après `k` propositions, c'est-à-dire de la dernière proposition marquée
`is_new_best` parmi les `k` premières du journal. L'effet est la part de l'excès de
Best-Fit supprimée, `(BF − champion) / (BF − L1)`. Un champion invalide sur la validation
compte pour −100 %.

**Écart.** Pour une seed et un palier : effet de l'évolution moins effet du tirage.

**Test principal (H3, effet d'échelle).** Écart à 1 359 appels contre écart à 340 appels,
sur les 12 seeds, test de Wilcoxon signé apparié, bilatéral, exact. H3 est établie si
p < 0,05 et si l'écart grandit sur plus de seeds qu'il ne se réduit.

**Tests secondaires, même test et même seuil.**

- H1, à appels égaux : évolution contre tirage à 1 359 appels.
- H2, à dépense égale : le rapport `r` est le coût par appel de l'évolution divisé par
  celui du tirage, mesuré sur les 24 runs de ce protocole. L'évolution arrêtée à
  `k* = ⌊1 359 / r⌋` appels est comparée au tirage complet de 1 359 propositions. H2 n'est
  interprétée que si H1 est établie.

**Lecture.**

- H3 établie : l'avance de l'évolution grandit avec le budget, sur ce problème et ce
  modèle.
- H3 significatif dans l'autre sens : l'avance se réduit quand le budget augmente.
- H3 non établie : pas de tendance montrée avec 12 seeds.
- H1 non établie à 1 359 appels : l'avantage observé à 85 et 340 appels n'est pas retrouvé.

**Extension prévue.** Si H3 n'est pas établie mais que p < 0,20 avec un écart qui grandit
sur plus de seeds qu'il ne se réduit, 12 seeds supplémentaires (8204 à 8391) peuvent être
exécutées à l'identique. Le test final porte alors sur les 24 seeds, au seuil de 0,025 au
lieu de 0,05, pour tenir compte du second regard. Aucune autre extension.

**Mesures descriptives, sans valeur de test.**

- Effet médian sur la validation et à l'entraînement, par bras et par palier ; nombre de
  seeds à 10 % ou plus et à 30 % ou plus.
- Écart médian et nombre de seeds gagnées par l'évolution, par palier.
- Budget de tirage équivalent : pour chaque palier `k` de l'évolution, le plus petit
  nombre de propositions auquel la médiane du tirage (validation) atteint la médiane de
  l'évolution à `k` ; « plus de 1 359 » si elle ne l'atteint pas.
- Dernier appel ayant amélioré le champion, par bras.
- Part des champions de l'évolution issus d'une proposition fraîche de relance.
- Tokens et coût par bras, rapport de coût `r`.

## Limites connues

- Un seul modèle, un seul problème.
- 12 seeds : le test ne peut voir qu'un effet net.
- Le champion à un palier est choisi sur l'entraînement ; sa note de validation peut
  baisser d'un palier au suivant.
- Les paliers d'une même seed ne sont pas indépendants : ils viennent du même run.
- Sans champ `seed`, les réponses du modèle ne sont pas reproductibles.
- Un quart environ des appels de l'évolution sont des propositions fraîches de relance
  (23 % et 29 % sur les deux runs de l'essai préalable) : le bras d'évolution contient une
  part de tirage.
- Les propositions fraîches reçoivent un des cinq « archétypes » du prompt de génération,
  dont un proche de l'astuce de la référence manuelle. Le prompt de mutation n'en contient
  pas.
- L'interpréteur corrigé garde des tolérances documentées : un champion peut ne pas être du
  Rust compilable.
- Les trois modifications du moteur (départ large, relance, mutation informée) sont testées
  ensemble.
