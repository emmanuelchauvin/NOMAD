# Résultats : évolution avec élagage contre tirage, à dépense égale, sur glm-5.3-flash

Protocole : [PROTOCOLE_GLM_H2_ELAGAGE.md](PROTOCOLE_GLM_H2_ELAGAGE.md), fixé avant exécution
(commit `e92da76`). Tir du 16 novembre 2026, 20 h 30, au 17 novembre, 2 h 58, sur le laptop.
Données brutes : `results_glm_h2/` (commit `0423752`, commitées avant lecture). Dépouillement :
`python scripts/analysis/glm_h2.py`.

## Conclusion

**H1 et H2 sont établies.** Avec l'élagage du champion, l'évolution fait mieux que le tirage
de propositions indépendantes sur glm-5.3-flash, à appels égaux et à dépense égale.

| Test | Comparaison | Seeds gagnées / perdues | Écart médian | p (seuil 0,05) |
|---|---|---|---|---|
| H1 | évolution à 344 appels contre tirage à 344 | 20 / 4 | +28,1 points | < 0,0001 (W+ = 284) |
| H2 | évolution à 162 appels contre tirage à 344 | 16 / 8 | +15,5 points | 0,0138 (W+ = 235) |

Wilcoxon signé apparié, bilatéral, exact, 24 seeds.

Dans le protocole à 344 appels sans élagage ([RESULTATS_GLM_340.md](RESULTATS_GLM_340.md)),
H1 était établie mais pas H2 : un appel d'évolution coûtait 4,36 fois un appel de tirage.
Ici, le rapport tombe à 2,12. Le protocole lit ce résultat comme « l'échec de H2 venait du
coût par appel ». La lecture est indirecte : l'exécutable diffère aussi par le croisement
corrigé (sans effet ici, le bras évolution ne croise pas) et par les gabarits de repli
comptés invalides (dans les deux bras).

## Contrôles

- 48 runs sur 48, 344 propositions du modèle chacun, consommation journalisée.
- Tirage : uniquement des propositions fraîches. Évolution : aucun croisement, élagage tenté
  sur chaque seed.
- Chaque champion a une note de validation. Aucun champion invalide sur la validation.
- Coût total : 2,03 $ (estimé 2,10 $).

## Coût par appel et point de dépense égale

| Bras | Requêtes | Tokens en entrée | Tokens en sortie | Coût | Par appel |
|---|---|---|---|---|---|
| Évolution | 8 256 | 15 343 344 | 9 790 693 | 1,38 $ | 0,000167 $ |
| Tirage | 8 256 | 5 434 944 | 5 532 607 | 0,65 $ | 0,000079 $ |

Tarif Relace : 0,0399 $ par million de tokens en entrée, 0,0784 $ en sortie.
`r = 2,1208`, donc `k* = ⌊344 / r⌋ = 162` (attendu : voisin de 156).

## Courbe (descriptif)

Effet : part de l'excès de Best-Fit supprimée, `(BF − champion) / (BF − L1)`, médiane sur
24 seeds.

**Validation**

| Appels | Évolution | Tirage | ≥ 30 % (évo / tir) | Seeds gagnées / perdues |
|---|---|---|---|---|
| 85 | 37,4 % | 21,0 % | 15 / 6 | 15 / 9 |
| 170 | 49,2 % | 28,8 % | 19 / 9 | 18 / 6 |
| 344 | 64,3 % | 32,6 % | 22 / 15 | 20 / 4 |

**Entraînement**

| Appels | Évolution | Tirage |
|---|---|---|
| 85 | 42,6 % | 21,6 % |
| 170 | 54,3 % | 26,4 % |
| 344 | 70,0 % | 29,2 % |

Le tirage plafonne après 170 appels ; l'évolution continue de monter. L'écart grandit avec
le budget, mais ce protocole ne teste pas H3.

**Excès sur la borne L1** (validation, médiane, en % de L1) : Best-Fit 4,65 % ; à 344
appels, évolution 1,68 %, tirage 3,14 %.

### Par seed (validation, effet en %)

| Seed | Évo 85 | Tir 85 | Évo 170 | Tir 170 | Évo 344 | Tir 344 |
|---|---|---|---|---|---|---|
| 10000 | 2,2 | 25,0 | 49,4 | 25,0 | 65,6 | 25,0 |
| 10017 | 38,2 | 22,0 | 38,2 | 22,0 | 36,6 | 22,0 |
| 10034 | 63,0 | 29,2 | 64,1 | 29,2 | 64,6 | 29,2 |
| 10051 | 4,2 | 48,7 | 3,7 | 48,7 | 53,9 | 48,7 |
| 10068 | 66,0 | 29,3 | 71,8 | 29,3 | 76,1 | 36,2 |
| 10085 | 37,0 | 12,2 | 60,8 | 39,2 | 60,8 | 39,2 |
| 10102 | 22,5 | 3,1 | 24,1 | 14,7 | 22,5 | 31,9 |
| 10119 | 37,8 | 30,3 | 37,2 | 25,0 | 37,2 | 25,5 |
| 10136 | 35,8 | 15,1 | 35,8 | 39,7 | 30,7 | 39,7 |
| 10153 | 0,5 | 3,6 | 2,6 | 35,7 | 57,1 | 30,1 |
| 10170 | −2,1 | 15,4 | 71,3 | 23,4 | 72,3 | 23,4 |
| 10187 | 0,0 | 38,1 | 52,4 | 45,0 | 70,9 | 45,0 |
| 10204 | 15,4 | 15,9 | 19,2 | 36,8 | 29,1 | 36,8 |
| 10221 | 62,3 | 4,6 | 67,4 | 4,6 | 67,4 | 38,3 |
| 10238 | 48,4 | 61,5 | 60,9 | 61,5 | 67,2 | 61,5 |
| 10255 | 53,3 | 20,1 | 59,8 | 20,1 | 61,4 | 20,1 |
| 10272 | 4,9 | 11,5 | 7,1 | 11,5 | 33,3 | 11,5 |
| 10289 | 54,6 | 30,8 | 68,6 | 30,8 | 79,5 | 30,8 |
| 10306 | 47,1 | 3,7 | 48,1 | 29,1 | 64,0 | 33,3 |
| 10323 | 33,2 | 2,2 | 48,9 | 2,2 | 72,8 | 37,5 |
| 10340 | 73,8 | 34,8 | 72,7 | 34,8 | 73,3 | 34,8 |
| 10357 | 45,7 | 11,7 | 47,3 | 26,6 | 65,4 | 26,6 |
| 10374 | 5,5 | 28,4 | 38,8 | 28,4 | 47,5 | 49,2 |
| 10391 | 73,6 | 22,5 | 78,6 | 22,5 | 78,6 | 22,5 |

Le champion d'évolution peut perdre un peu sur la validation d'un palier à l'autre (10017,
10102, 10136) : il est choisi sur l'entraînement.

## Autres mesures descriptives

- **Élagage** : 266 tentatives, 148 acceptées, 441 instructions retirées, 2 884 évaluations
  supplémentaires (non facturées). Le champion final est issu d'un élagage dans 8 seeds sur 24.
- **Taille du code du champion final** (médiane) : évolution 3 570 caractères, tirage 2 104.
- **Propositions invalides à l'entraînement** : évolution 935 / 8 256 (11,3 %), tirage
  1 079 / 8 256 (13,1 %). Ces taux comptent désormais les gabarits de repli.

## Limites

- Un seul problème (bin packing, profil Weibull), un seul modèle servi en fp4 ; réponses non
  reproductibles.
- L'exécutable diffère de celui du protocole à 344 appels : les chiffres ne se comparent pas
  directement d'un protocole à l'autre.
- Le réglage du moteur a été choisi sur Gemini.
- L'élagage fait des évaluations supplémentaires sur l'entraînement, gratuites en appels au
  modèle mais pas en calcul.
