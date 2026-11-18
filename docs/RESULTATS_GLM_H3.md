# Résultats : effet d'échelle sur glm-5.3-flash, évolution avec élagage contre tirage jusqu'à 1 359 appels

Protocole : [PROTOCOLE_GLM_H3.md](PROTOCOLE_GLM_H3.md), fixé avant exécution (commit
`62e3eaf`). Essai de fumée : commit `15e7d75`. Tir du 17 novembre 2026, 9 h 02, au
18 novembre, 14 h 02, sur le laptop (29 h ; estimé 20 à 30 h). Données brutes :
`results_glm_h3/` (commit `41029bc`, commitées avant lecture). Dépouillement :
`python scripts/analysis/glm_h3.py`.

## Conclusion

**H3 n'est pas établie. H1 et H2 le sont, à 1 359 appels.** Sur glm-5.3-flash,
l'évolution avec élagage fait mieux que le tirage de propositions indépendantes à appels
égaux et à dépense égale jusqu'à 1 359 appels, mais son avance ne grandit pas entre 344 et
1 359 appels.

| Test | Comparaison | Seeds | Écart médian | p (seuil 0,05) | Verdict |
|---|---|---|---|---|---|
| **H3** (principal) | écart à 1 359 appels contre écart à 344 | l'écart grandit sur 13, se réduit sur 11 | +3,9 points | 0,60 (W+ = 169) | non établie |
| H1 | évolution contre tirage, 1 359 appels chacun | 24 gagnées, 0 perdue | +25,9 points | < 0,0001 (W+ = 300) | établie |
| H2 | évolution à 393 appels contre tirage à 1 359 | 19 gagnées, 5 perdues | +13,5 points | 0,0065 (W+ = 243) | établie |

Wilcoxon signé apparié, bilatéral, exact, 24 seeds. Lecture fixée par le protocole pour
H3 : pas de tendance montrée avec 24 seeds.

**Contraste avec Gemini.** Sur gemini-3.1-flash-lite, H3 est établie
([RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md) : l'écart grandit sur 16 seeds,
se réduit sur 6, p = 0,0042), parce que le tirage y plafonne (17,5 % à 680 propositions,
17,7 % à 1 359) pendant que l'évolution continue. Sur GLM, le tirage continue de
progresser (37,7 % puis 42,5 %) et l'évolution ralentit (63,0 % puis 65,4 %) : l'écart
médian reste entre 20 et 27 points de 85 à 1 359 appels. Sur ces deux modèles, l'avance de
l'évolution ne grandit avec le budget que là où le tirage s'épuise.

**Ce qui progresse avec le budget, c'est la régularité.** L'évolution gagne 17 seeds sur
24 à 344 appels, 22 à 680 et les 24 à 1 359 (22 sur 24 sur Gemini) : c'est le résultat H1
le plus net du projet.

## Contrôles

- 48 runs sur 48, 1 359 propositions du modèle chacun, consommation journalisée.
- Tirage : uniquement des propositions fraîches. Évolution : aucun croisement, élagage
  tenté sur chaque seed.
- Chaque champion a une note de validation. Aucun champion invalide sur la validation, à
  aucun palier.
- **Deux runs relancés depuis le début** par le lanceur, qui prévoit trois essais par
  seed. Les premiers essais n'ont pas été lus.
  - Évolution, seed 10689 : le premier essai s'est arrêté après 245 appels, OpenRouter
    ayant refusé six fois de suite le même appel (erreur 429 : l'hébergeur Relace était
    saturé).
  - Tirage, seed 10417 : le premier essai s'est arrêté après ses 1 359 appels, sur une
    erreur du programme (« user-provided comparison function does not correctly implement
    a total order », code 101). Cause probable, non reproduite : l'ordre de
    `BenchmarkScore` (`crates/nomad-core/src/metric.rs`) tient deux `tldr_score` pour
    égaux à 1e-4 près puis passe aux critères suivants, ce qui ne donne pas un ordre total ;
    le tri des candidats en fin de génération (`engine.rs`) peut le détecter et s'arrêter.
    À corriger avant le prochain exécutable figé. *Ajout du 18 novembre, après ce
    dépouillement : cause confirmée par un test qui reproduit l'arrêt (même message, même
    endroit), corrigée sur `ast-croisement` (commit `dff2084`). Les résultats ci-dessus,
    obtenus avec l'exécutable figé, ne changent pas.*
- **Dépense : 11,90 $** (estimée 10 à 15 $), dont 0,23 $ pour les deux essais ratés.

## Coût par appel et point de dépense égale

| Bras | Requêtes | Tokens en entrée | Tokens en sortie | Coût | Par appel |
|---|---|---|---|---|---|
| Évolution | 32 616 | 97 533 991 | 65 871 113 | 9,06 $ | 0,000278 $ |
| Tirage | 32 616 | 21 487 536 | 22 511 877 | 2,62 $ | 0,000080 $ |

Runs complets seulement, au tarif de Relace (0,0399 $ par million de tokens en entrée,
0,0784 $ en sortie). `r = 3,4534`, donc `k* = ⌊1 359 / r⌋ = 393`. Un run d'évolution
complet coûte en moyenne 0,377 $, un run de tirage 0,109 $.

**Écart entre le script et le protocole, corrigé après lecture.** Le protocole mesure `r`
« sur les 48 runs complets ». Le script commité avant le tir comptait aussi les tokens des
deux essais ratés (`r = 3,4775`, `k* = 390`). Il a été corrigé pour suivre le protocole.
La conclusion ne change pas : à 390 appels aussi, l'évolution gagne 19 seeds et en perd 5
(p = 0,0065).

**Coût par appel selon le rang de l'appel dans le run**

| Appels | 1–85 | 86–170 | 171–344 | 345–680 | 681–1 359 |
|---|---|---|---|---|---|
| Évolution | 0,000114 $ | 0,000164 $ | 0,000209 $ | 0,000280 $ | 0,000329 $ |
| Tirage | 0,000081 $ | 0,000080 $ | 0,000079 $ | 0,000083 $ | 0,000079 $ |

Le coût d'un appel d'évolution triple au fil du run, sans plafond visible ; celui du
tirage est constant.

**Le test H2 est défavorable à l'évolution** (calcul fait après lecture, sans valeur de
test). `k*` divise le budget par le coût moyen d'un appel sur tout le run, alors que les
premiers appels d'évolution coûtent moins que les derniers. À 393 appels, l'évolution a
dépensé en moyenne 0,072 $ par run, 66 % du tirage complet (0,109 $) ; sa dépense
cumulée n'égale celle du tirage complet qu'à 529 appels. Il en allait de même dans le
protocole H2 : à `k*` = 162, 78 % de la dépense du tirage, égalité à 197 appels.

## Courbe (descriptif)

Effet : part de l'excès de Best-Fit supprimée, `(BF − champion) / (BF − L1)`, médiane sur
24 seeds.

**Validation**

| Appels | Évolution | Tirage | Écart médian | Seeds gagnées / perdues | ≥ 30 % (évo / tir) |
|---|---|---|---|---|---|
| 85 | 30,5 % | 14,7 % | 20,2 | 16 / 8 | 12 / 4 |
| 170 | 50,5 % | 21,7 % | 25,5 | 16 / 8 | 15 / 8 |
| 344 | 57,2 % | 31,6 % | 22,5 | 17 / 6 (1 égalité) | 20 / 15 |
| 680 | 63,0 % | 37,7 % | 26,9 | 22 / 2 | 23 / 18 |
| 1 359 | 65,4 % | 42,5 % | 25,9 | 24 / 0 | 24 / 23 |

**Entraînement**

| Appels | Évolution | Tirage |
|---|---|---|
| 85 | 29,0 % | 12,0 % |
| 170 | 51,2 % | 20,3 % |
| 344 | 59,7 % | 31,1 % |
| 680 | 64,3 % | 35,3 % |
| 1 359 | 71,4 % | 45,1 % |

**Excès sur la borne L1** (validation, médiane, en % de L1 ; Best-Fit : 4,67 %)

| Appels | 85 | 170 | 344 | 680 | 1 359 |
|---|---|---|---|---|---|
| Évolution | 3,14 % | 2,32 % | 1,96 % | 1,75 % | 1,61 % |
| Tirage | 4,02 % | 3,68 % | 3,20 % | 2,95 % | 2,67 % |

### Par seed (validation, effet en %)

| Seed | Évo 85 | Tir 85 | Évo 170 | Tir 170 | Évo 344 | Tir 344 | Évo 680 | Tir 680 | Évo 1 359 | Tir 1 359 |
|---|---|---|---|---|---|---|---|---|---|---|
| 10400 | 60,7 | 29,5 | 60,7 | 29,5 | 65,6 | 29,5 | 64,5 | 33,3 | 66,1 | 33,3 |
| 10417 | 12,0 | 4,9 | 53,8 | 4,9 | 65,2 | 2,2 | 71,7 | 10,3 | 72,8 | 37,5 |
| 10434 | 36,7 | 3,2 | 44,7 | 21,3 | 42,0 | 21,3 | 52,1 | 42,0 | 80,9 | 28,7 |
| 10451 | −1,6 | 22,6 | −0,5 | 14,0 | 27,4 | 28,5 | 58,1 | 28,5 | 59,7 | 36,0 |
| 10468 | 14,0 | 43,0 | 17,7 | 46,2 | 17,7 | 46,2 | 25,8 | 46,2 | 65,1 | 39,8 |
| 10485 | 48,9 | 18,1 | 52,7 | 20,9 | 52,7 | 35,2 | 53,8 | 47,3 | 56,6 | 47,3 |
| 10502 | 50,3 | 1,6 | 67,0 | 7,0 | 64,9 | 42,7 | 68,6 | 42,7 | 70,3 | 42,7 |
| 10519 | 58,0 | 34,6 | 57,4 | 34,6 | 57,4 | 34,6 | 62,8 | 37,8 | 64,4 | 59,6 |
| 10536 | 7,7 | 23,6 | 63,7 | 23,6 | 72,5 | 41,8 | 73,1 | 41,8 | 75,8 | 41,8 |
| 10553 | 24,3 | 7,3 | 29,4 | 40,1 | 31,6 | 32,2 | 49,2 | 29,9 | 63,8 | 31,1 |
| 10570 | 1,0 | 6,8 | −0,5 | 9,9 | 35,6 | 9,9 | 69,1 | 37,7 | 68,1 | 37,7 |
| 10587 | 56,4 | 3,6 | 56,4 | 11,8 | 56,9 | 11,8 | 56,9 | 39,5 | 60,0 | 39,5 |
| 10604 | 7,7 | 23,0 | 7,7 | 23,0 | 61,2 | 39,3 | 66,3 | 39,3 | 65,8 | 39,3 |
| 10621 | 19,9 | 15,2 | 29,3 | 31,4 | 31,4 | 31,4 | 31,9 | 31,4 | 53,9 | 44,5 |
| 10638 | 0,5 | 3,6 | 39,1 | 11,5 | 83,3 | 33,3 | 80,7 | 39,6 | 80,7 | 43,8 |
| 10655 | 61,1 | 31,1 | 63,7 | 31,1 | 62,1 | 31,1 | 63,2 | 36,3 | 63,2 | 50,5 |
| 10672 | 44,1 | 7,2 | 48,2 | 31,8 | 60,5 | 31,8 | 60,5 | 31,8 | 60,5 | 42,6 |
| 10689 | 41,8 | −2,3 | 53,1 | 12,4 | 55,9 | 24,3 | 55,9 | 19,2 | 56,5 | 42,4 |
| 10706 | 17,3 | 11,9 | 17,3 | 11,9 | 16,8 | 45,4 | 42,7 | 45,4 | 57,8 | 43,8 |
| 10723 | 52,6 | 13,9 | 78,4 | 13,9 | 78,9 | 10,3 | 79,9 | 26,8 | 80,9 | 45,9 |
| 10740 | 41,4 | 14,1 | 60,2 | 14,1 | 60,2 | 27,2 | 74,9 | 27,7 | 73,8 | 66,0 |
| 10757 | 5,1 | 65,1 | 26,7 | 65,1 | 43,6 | 65,1 | 66,7 | 65,1 | 70,3 | 65,1 |
| 10774 | 1,1 | 20,0 | 1,1 | 30,8 | 1,6 | 30,8 | 55,1 | 30,8 | 57,3 | 30,8 |
| 10791 | 45,5 | 21,2 | 58,7 | 22,2 | 63,0 | 45,0 | 72,5 | 45,0 | 72,5 | 45,0 |

Le champion d'un palier est choisi sur l'entraînement : sa note de validation peut baisser
d'un palier au suivant (évolution 10638 : 83,3 % puis 80,7 % ; tirage 10468 : 46,2 % puis
39,8 %).

## Observations (descriptif, sans valeur de test)

1. **L'écart se resserre autour de sa médiane au lieu de grandir.** Entre 344 et 1 359
   appels, il grandit sur les 7 seeds où l'évolution était derrière ou à égalité à 344
   appels, et l'évolution les gagne toutes à 1 359 (10774 : 1,6 % puis 57,3 %, contre
   30,8 % au tirage ; 10468 : 17,7 % puis 65,1 % ; 10706 : 16,8 % puis 57,8 % ; 10757 :
   43,6 % puis 70,3 %, contre 65,1 %). Il se réduit sur 8 des 9 seeds où l'évolution avait
   au moins 30 points d'avance, surtout parce que le tirage y trouve tard une bonne
   heuristique (10723 : tirage 10,3 % puis 45,9 % ; 10740 : 27,2 % puis 66,0 % ; 10417 :
   2,2 % puis 37,5 %). L'avance médiane ne bouge presque pas, mais elle s'étend à toutes
   les seeds.
2. **Le code grossit.** Le champion final de l'évolution fait 9 120 caractères en médiane
   (3 570 à 344 appels dans le protocole H2 ; 2 101 pour le tirage). C'est ce qui fait
   tripler le coût d'un appel. L'élagage en une passe ne l'arrête pas : 254 élagages
   acceptés sur 608 tentatives, champion final issu d'un élagage dans 3 seeds sur 24. Que
   ce grossissement freine aussi la progression de l'évolution est une hypothèse, non
   testée.
3. **À 344 appels, ces 24 nouvelles seeds redonnent l'image du protocole H2** : évolution
   57,2 %, tirage 31,6 %, 17 seeds gagnées et 6 perdues (protocole H2 : 64,3 % et 32,6 %,
   20 seeds gagnées et 4 perdues).

## Autres mesures descriptives

- **Élagage** : 608 tentatives, 254 acceptées, 687 instructions retirées, 8 241
  évaluations supplémentaires (non facturées).
- **Propositions invalides à l'entraînement** : évolution 4 157 / 32 616 (12,7 %), tirage
  4 313 / 32 616 (13,2 %).

## Conséquences pour la suite

- **Preuve sur GLM** : H1 et H2 établies à 344 appels
  ([RESULTATS_GLM_H2.md](RESULTATS_GLM_H2.md)) et à 1 359 appels ; H3 non établie. Sur
  Gemini, H1, H2 et H3 sont établies.
- **Bug du tri** à corriger avant le prochain exécutable figé (voir Contrôles).
- **Taille du code** : c'est elle qui fait monter le coût de l'évolution sur GLM. Pistes :
  une greffe qui remplace au lieu d'empiler, un élagage plus fort.
- **Point de dépense égale** : pour les prochains protocoles, le définir par la dépense
  cumulée plutôt que par le coût moyen par appel serait plus juste. À décider, et à fixer
  avant exécution.

## Limites

- Un seul problème (bin packing, profil Weibull), un seul modèle servi en fp4 ; réponses
  non reproductibles.
- Les paliers d'une même seed ne sont pas indépendants : ils viennent du même run.
- Le champion d'un palier est choisi sur l'entraînement.
- `k*` est calculé avec le coût moyen par appel : le test H2 est défavorable à
  l'évolution.
- Deux runs ont été relancés depuis le début après un échec ; leurs premiers essais n'ont
  pas été lus.
- Une part des appels de l'évolution sont des propositions fraîches de relance : le bras
  d'évolution contient une part de tirage.
- Le réglage du moteur a été choisi sur Gemini.
- L'élagage fait des évaluations supplémentaires sur l'entraînement, gratuites en appels
  au modèle mais pas en calcul.
