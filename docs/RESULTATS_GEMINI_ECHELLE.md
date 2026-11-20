# Résultats du protocole d'échelle sur Gemini (340 appels)

Runs exécutés le 14 novembre 2026, de 0 h 44 à 6 h 53, selon
[PROTOCOLE_GEMINI_ECHELLE.md](PROTOCOLE_GEMINI_ECHELLE.md), avec l'exécutable figé par le
tag `gemini-echelle`. Les 72 runs prévus (3 bras de 24 seeds) sont complets, sans échec ni
nouvel essai. Données brutes : `results_gemini_echelle/`, commitées avant le dépouillement.

Pour reproduire les tableaux et les tests :

```
python scripts/analysis/gemini_echelle.py
```

## Verdict

**H1 et H2 sont établies. H3 ne l'est pas.**

- **H1, à appels égaux (340 contre 340).** L'évolution fait mieux que le tirage : elle gagne
  20 seeds et en perd 4. Wilcoxon signé apparié, bilatéral, exact : W+ = 270, n = 24,
  p = 0,0002. Différence médiane : 10,1 points.
- **H2, à dépense égale (340 appels contre 455 propositions).** L'évolution fait mieux :
  elle gagne 19 seeds et en perd 5. W+ = 256, n = 24, p = 0,0016. Différence médiane :
  17,2 points. Les deux bras ont coûté presque le même prix (11,64 $ et 11,41 $).
- **H3, l'écart grandit-il avec le budget ?** Non montré. L'écart médian par seed entre
  évolution et tirage à appels égaux est de 10,1 points à 340 appels, contre 14,1 points à
  85 appels. Mann-Whitney bilatéral : U = 329, p = 0,40.

Lecture prévue par le protocole : l'avantage de l'évolution tient à un budget quatre fois
plus grand, y compris à dépense égale. Ce protocole ne montre pas que l'écart grandit ; il
ne montre pas non plus qu'il ne grandit pas.

## Mesures

| Mesure | Tirage 340 | Évolution 340 | Tirage 455 |
|---|---|---|---|
| Effet médian sur la validation | 16,3 % | 32,5 % | 13,5 % |
| Succès (≥ 10 %) sur la validation | 19/24 | 24/24 | 21/24 |
| Succès (≥ 30 %) sur la validation | 2/24 | 13/24 | 3/24 |
| Effet médian sur OOD-1 | 24,1 % | 24,1 % | 24,1 % |
| Succès (≥ 10 %) sur OOD-1 | 19/24 | 23/24 | 21/24 |
| Succès (≥ 30 %) sur OOD-1 | 8/24 | 11/24 | 6/24 |
| Effet médian sur OOD-2 | 6,7 % | 19,2 % | 14,1 % |
| Succès (≥ 10 %) sur OOD-2 | 10/24 | 22/24 | 15/24 |
| Succès (≥ 30 %) sur OOD-2 | 0/24 | 7/24 | 1/24 |
| Champions invalides | 0 | 0 | 0 |
| Appels par run | 340 | 340 | 455 |
| Tokens en entrée | 6,03 millions | 10,89 millions | 8,07 millions |
| Tokens en sortie | 4,69 millions | 5,94 millions | 6,26 millions |
| Coût estimé | 8,54 $ | 11,64 $ | 11,41 $ |

Coût estimé au tarif retenu par le script (0,25 $ par million de tokens en entrée, 1,50 $
en sortie). Total des trois bras : 31,59 $, plus 1,43 $ pour l'essai préalable.

Rapport de coût mesuré : l'évolution a coûté 1,36 fois le tirage à 340 appels et 1,02 fois
le tirage à 455 propositions. Le rapport prévu par l'essai préalable était de 1,34.

Aucun champion de l'évolution n'est une proposition de départ non modifiée (0 sur 24,
contre 3 sur 24 à 85 appels).

## Excès de Best-Fit supprimé sur la validation, par seed

| Seed | Tirage 340 | Évolution 340 | Tirage 455 |
|---|---|---|---|
| 7000 | 4,5 % | 13,4 % | 1,7 % |
| 7017 | 7,5 % | 10,7 % | 12,8 % |
| 7034 | 24,6 % | 49,2 % | 30,4 % |
| 7051 | 20,3 % | 31,6 % | 16,0 % |
| 7068 | 20,1 % | 16,8 % | 25,7 % |
| 7085 | 2,6 % | 40,5 % | 1,6 % |
| 7102 | 20,5 % | 21,6 % | 70,5 % |
| 7119 | 16,8 % | 13,7 % | 10,7 % |
| 7136 | 10,6 % | 63,8 % | 13,8 % |
| 7153 | 16,5 % | 33,5 % | 10,1 % |
| 7170 | 40,1 % | 44,4 % | 18,7 % |
| 7187 | 21,8 % | 14,9 % | 19,1 % |
| 7204 | 7,6 % | 16,2 % | 12,4 % |
| 7221 | 46,6 % | 63,5 % | 12,7 % |
| 7238 | 18,0 % | 25,9 % | 29,1 % |
| 7255 | 16,1 % | 69,8 % | 34,9 % |
| 7272 | 14,4 % | 46,4 % | 3,6 % |
| 7289 | 16,5 % | 54,6 % | 10,8 % |
| 7306 | 2,2 % | 51,9 % | 13,1 % |
| 7323 | 29,3 % | 19,6 % | 15,8 % |
| 7340 | 14,8 % | 49,7 % | 15,3 % |
| 7357 | 12,1 % | 62,1 % | 14,2 % |
| 7374 | 14,0 % | 16,1 % | 10,8 % |
| 7391 | 12,9 % | 13,4 % | 11,9 % |

## Comparaison avec 85 appels

Les deux protocoles n'ont pas les mêmes seeds : ce tableau rapproche deux expériences, il
ne les apparie pas.

| Sur la validation | Tirage 85 | Évolution 85 | Tirage 340 | Évolution 340 |
|---|---|---|---|---|
| Effet médian | 4,6 % | 15,2 % | 16,3 % | 32,5 % |
| Succès (≥ 10 %) | 6/24 | 14/24 | 19/24 | 24/24 |
| Succès (≥ 30 %) | 1/24 | 8/24 | 2/24 | 13/24 |
| Seeds gagnées par l'évolution | | 15 sur 24 | | 20 sur 24 |

## Observations

1. **Les deux procédés progressent avec le budget, à peu près du même pas.** L'effet médian
   du tirage passe de 4,6 % à 16,3 %, celui de l'évolution de 15,2 % à 32,5 %. L'écart
   entre médianes de bras s'élargit (de 10,6 à 16,2 points) alors que l'écart médian par
   seed, la mesure fixée par le protocole, se réduit (de 14,1 à 10,1 points). Les deux
   lectures divergent : c'est le signe qu'aucune tendance nette n'est mesurée, ce que dit
   le test (p = 0,40).
2. **L'évolution est devenue régulière.** À 85 appels, elle restait sous 10 % sur 10 seeds
   sur 24. À 340 appels, elle est au-dessus de 10 % sur les 24 seeds et gagne 20 fois sur
   24. Le « régime bas » observé à 85 appels a disparu.
3. **Le tirage plafonne.** Passer de 340 à 455 propositions n'améliore pas la validation
   (médiane 13,5 % contre 16,3 %) et le nombre de seeds au-dessus de 30 % reste à 2 ou 3.
   Ajouter des propositions indépendantes relève le bas mais ne produit pas de fortes
   valeurs, comme à 85 et 170.
4. **L'avantage se retrouve cette fois sur OOD-2.** L'évolution y a la meilleure médiane
   (19,2 % contre 6,7 % et 14,1 %), 22 seeds sur 24 au-dessus de 10 % et un seul champion
   négatif (−8,7 %). Les plus mauvais champions sont du côté des tirages (−76,1 % et
   −53,7 %). C'est l'inverse de l'observation faite à 85 appels, où l'évolution avait 6
   champions sous −10 % sur OOD-2. Mesures descriptives.
5. **Sur OOD-1, les trois bras ont la même médiane** (24,1 %, coïncidence d'arrondi sur
   trois valeurs voisines). L'évolution y a un peu plus de seeds au-dessus de 30 % (11
   contre 8 et 6), sans écart net.
6. **Le budget à dépense égale était bien calé.** L'essai préalable sur trois seeds a
   prévu le rapport de coût à 2 % près.

## Limites

Celles du protocole restent entières : un seul modèle, un seul problème ; réponses non
reproductibles faute de champ `seed` ; les trois modifications du moteur sont testées
ensemble ; H3 compare deux expériences faites à un jour d'écart, sur des seeds différentes,
et ne peut voir qu'un changement important.

## Lecture d'ensemble

Sur deux budgets et 48 seeds, avec des critères fixés à l'avance, l'évolution de NOMAD fait
mieux qu'un tirage de propositions indépendantes du même modèle : à 85 appels (p = 0,0075),
à 340 appels (p = 0,0002), et à 340 appels contre un tirage du même prix (p = 0,0016). La
question « l'évolution vaut-elle mieux que le tirage ? » a une réponse positive sur ce
problème et ce modèle.

La question « l'écart grandit-il avec l'échelle ? » n'en a pas encore. Deux budgets ne
suffisent pas à mesurer une tendance, et le tirage progresse lui aussi quand on lui donne
plus de propositions. Restent aussi à établir : le même résultat sur un autre modèle et sur
un autre problème.
