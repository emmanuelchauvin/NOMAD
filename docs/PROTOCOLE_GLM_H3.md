# Protocole : effet d'échelle sur glm-5.3-flash, évolution avec élagage contre tirage jusqu'à 1 359 appels (fixé avant exécution)

Rédigé le 17 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Sur glm-5.3-flash, l'évolution avec élagage du champion bat le tirage de propositions
indépendantes à appels égaux et à dépense égale, à 344 appels
([RESULTATS_GLM_H2.md](RESULTATS_GLM_H2.md)). Sur Gemini, l'avance de l'évolution grandit
avec le budget ([RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md)). Est-ce aussi le
cas sur GLM ?

- **H3 (test principal)** : l'avance de l'évolution sur le tirage est plus grande à
  1 359 appels qu'à 344.
- H1 (secondaire) : à 1 359 appels, l'évolution fait mieux que le tirage.
- H2 (secondaire) : à dépense égale aussi.

## Principe

Chaque run enregistre toutes ses propositions dans l'ordre et note sur la validation chacun
de ses champions successifs. Un run de 1 359 appels donne donc aussi son champion après 85,
170, 344 ou 680 appels : l'avance est mesurée aux deux budgets **sur les mêmes seeds, dans
les mêmes runs**, comme dans le protocole de courbe sur Gemini. Un run long arrêté en route
est un run court (test `test_a_long_run_cut_short_is_the_short_run`).

## Dispositif

Identique à [PROTOCOLE_GLM_H2_ELAGAGE.md](PROTOCOLE_GLM_H2_ELAGAGE.md), sauf le budget et
les seeds.

- **Modèle.** `z-ai/glm-5.3-flash` par OpenRouter, hébergeur unique Relace (fp4) sans
  repli, raisonnement au minimum, pas de champ `seed`, température 0,7, top_p 0,95.
- **Exécutable.** `target/frozen/nomad-h2-elagage.exe`, le même que pour le protocole H2 :
  branche `ast-croisement`, tag `glm-h2-elagage` (commit `7caeeb1`), SHA-256
  `9e6e3098f077973970b48d8f0ff50d5b5822a8f633176f1818c8481420dae99b`. Les deux bras
  utilisent le même exécutable.
- **Benchmark.** Profil Weibull, fuel 400 000.
- **Seeds.** 24 seeds neuves : 10400, 10417, ..., 10791 (10400 + 17 r). Aucune n'a servi.

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 1 359 propositions fraîches | `run -G D --pop-size 1359 -n 1` |
| Évolution | Groupe D : 22 propositions fraîches puis 191 générations de 7 appels (1 359 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée, élagage de chaque nouveau champion (`NOMAD_PRUNE_CHAMPION=1`), sans croisement local | `run -G D --pop-size 8 -n 192`, `NOMAD_INIT_POP=22` |

**Lancement.** `scripts/campaign/lancer_glm_h3.ps1` : évolution, un processus par seed ;
tirage, douze processus de deux seeds. Le découpage ne change pas les résultats.

## Mesures et lecture, fixées à l'avance

**Paliers.** 85, 170, 344, 680 et 1 359 appels.

**Grandeur mesurée.** Pour une seed, un bras et un palier `k` : l'effet sur la validation
du champion après `k` appels. Le champion est la dernière proposition marquée `is_new_best`
dont le nombre d'appels déjà consommés est au plus `k` (les élagages, gratuits, comptent au
palier de l'appel qui les précède). Effet : part de l'excès de Best-Fit supprimée,
`(BF − champion) / (BF − L1)`. Un champion invalide sur la validation compte pour −100 %.

**Écart.** Pour une seed et un palier : effet de l'évolution moins effet du tirage.

**Tests.** Wilcoxon signé apparié, bilatéral, exact, sur les 24 seeds, seuil 0,05.

- **H3, test principal** : écart à 1 359 appels contre écart à 344 appels. Établie si
  p < 0,05 et si l'écart grandit sur plus de seeds qu'il ne se réduit.
- H1, secondaire : évolution contre tirage à 1 359 appels. Établie si p < 0,05 et si
  l'évolution gagne plus de seeds qu'elle n'en perd.
- H2, secondaire : `r` est le coût par appel de l'évolution divisé par celui du tirage,
  mesuré sur les 48 runs complets au tarif de Relace (0,0399 $ par million de tokens en
  entrée, 0,0784 $ en sortie). L'évolution arrêtée à `k* = ⌊1 359 / r⌋` appels contre le
  tirage complet (1 359 propositions). Même règle de décision. H2 n'est interprétée que si
  H1 est établie.

Aucune extension ni second regard : 24 seeds, un seul dépouillement.

**Lecture.**

- H3 établie : l'avance de l'évolution grandit avec le budget sur ce modèle aussi.
- H3 significative dans l'autre sens : l'avance se réduit quand le budget augmente.
- H3 non établie : pas de tendance montrée avec 24 seeds.
- H1 non établie à 1 359 appels : l'avantage observé à 344 appels n'est pas retrouvé.
- H1 établie, H2 non : à ce budget, le grossissement des programmes rend de nouveau
  l'évolution plus chère que ce qu'elle rapporte, malgré l'élagage.

**Mesures descriptives, sans valeur de test.** Médianes et succès (≥ 10 %, ≥ 30 %) par
palier, sur la validation et l'entraînement ; écart par palier ; excès sur la borne L1 en %
de L1 ; propositions invalides par bras ; taille du code du champion final ; compteurs de
l'élagage ; tokens, coût, `r` et `k*` ; coût par appel selon le rang de l'appel dans le
run.

**Dépouillement.** `python scripts/analysis/glm_h3.py`. Les données brutes sont commitées
avant d'être lues, et le script est d'abord passé en mode `controle`.

## Ce que l'on sait avant de lancer

Tiré du protocole H2, déjà dépouillé (24 autres seeds, 344 appels) :

- **Le coût d'un appel d'évolution monte au fil du run**, même avec l'élagage : 0,000112 $
  pour les appels 1 à 85, 0,000154 $ pour 86 à 170, 0,000200 $ pour 171 à 344. Celui du
  tirage est constant (0,000079 $). La hausse ralentit, sans plafond observé. `r`, qui
  valait 2,12 à 344 appels, sera donc plus grand ici, et `k*` plus bas en proportion : H2
  est plus difficile à établir à ce budget.
- **L'écart entre 170 et 344 appels** grandit sur 14 seeds, se réduit sur 8 (p = 0,20,
  calcul fait après coup sur les données H2 en écrivant ce script, sans valeur de test).
- **Le temps est celui de l'évaluation, pas du modèle** : 38 à 55 s par appel d'évolution
  et 18 à 23 s par appel de tirage avec 32 processus sur le laptop, dont 8 à 11 s et 6 à
  7 s d'attente du modèle.

## Coût et durée attendus

- **Coût.** Tirage : 24 × 1 359 × 0,000079 $, soit 2,60 $. Évolution : selon que le coût
  par appel se stabilise vers 0,0003 $ ou continue de monter, 8 à 13 $. **Total estimé :
  10 à 15 $.** L'essai de fumée (29 appels) ne permet pas de projeter ce coût.
- **Surveillance.** Pendant le tir, la dépense est suivie dans les journaux de consommation
  (tokens seulement, aucun score). Si la projection dépasse 20 $, l'utilisateur est prévenu
  et décide ; il n'y a pas d'arrêt automatique. Un tir interrompu n'est pas dépouillé.
- **Durée.** Estimée, non mesurée : 20 à 30 h sur le laptop, qui doit rester allumé,
  branché et sans mise en veille. Relancer le lanceur reprend les seeds non terminées ; un
  run interrompu repart de zéro (au plus 0,40 $ perdu pour un run d'évolution).

Un essai de fumée (`lancer_glm_h3.ps1 -Essai` : seed 3153, 29 appels par bras) vérifie la
clé, le réseau et l'exécutable. Ses scores ne sont pas lus ; on vérifie seulement que les
deux bras ont fait 29 appels et que l'évolution a tenté au moins un élagage.

## Limites connues

- Un seul problème, un seul modèle servi en fp4 ; réponses non reproductibles.
- Les paliers d'une même seed ne sont pas indépendants : ils viennent du même run.
- Le champion à un palier est choisi sur l'entraînement ; sa note de validation peut
  baisser d'un palier au suivant.
- `k*` est calculé avec le coût moyen par appel sur le run entier. Comme les premiers
  appels d'évolution coûtent moins que les derniers, l'évolution arrêtée à `k*` a en réalité
  dépensé moins que le tirage complet : le test H2 est défavorable à l'évolution. La
  définition est gardée pour rester comparable aux protocoles précédents.
- Une part des appels de l'évolution sont des propositions fraîches de relance : le bras
  d'évolution contient une part de tirage.
- Le réglage du moteur a été choisi sur Gemini.
- L'élagage fait des évaluations supplémentaires sur l'entraînement, non facturées.
