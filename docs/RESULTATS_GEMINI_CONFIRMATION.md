# Résultats du protocole de confirmation sur Gemini

Runs exécutés le 13 novembre 2026, de 20 h 08 à 21 h 51, selon
[PROTOCOLE_GEMINI_CONFIRMATION.md](PROTOCOLE_GEMINI_CONFIRMATION.md), avec l'exécutable figé
par le tag `gemini-v1`. Les 72 runs prévus (3 bras de 24 seeds) sont complets, sans échec ni
nouvel essai. Données brutes : `results_gemini_v1/`.

Pour reproduire les tableaux et les tests :

```
python scripts/analysis/gemini_confirmation.py
```

## Verdict

**H1 est établie, H2 ne l'est pas.**

- **H1, à appels égaux (85 contre 85).** L'évolution fait mieux que le tirage : elle gagne
  15 seeds, en perd 8, une égalité. Wilcoxon signé apparié, bilatéral, exact : W+ = 224,
  n = 23, p = 0,0075. Différence médiane : 14,1 points.
- **H2, contre le tirage de 170 propositions.** Pas de différence montrée : l'évolution
  gagne 13 seeds, en perd 11. W+ = 195, n = 24, p = 0,21. Différence médiane : 4,8 points.

Lecture prévue par le protocole pour ce cas : l'avantage de l'évolution à appels égaux ne
résiste pas à une comparaison à dépense égale.

Cette lecture doit être nuancée par un fait mesuré (observation 1) : le bras à 170 appels
a coûté 1,52 fois plus que l'évolution, et non autant. La comparaison H2 était donc plus
dure que « à dépense égale ». Le protocole ne permet pas d'en tirer plus : H2 reste non
établie.

C'est le premier résultat du projet qui passe un seuil fixé à l'avance.

## Mesures

| Mesure | Tirage 85 | Évolution | Tirage 170 |
|---|---|---|---|
| Effet médian sur la validation | 4,6 % | 15,2 % | 10,7 % |
| Succès (≥ 10 %) sur la validation | 6/24 | 14/24 | 14/24 |
| Succès (≥ 30 %) sur la validation | 1/24 | 8/24 | 1/24 |
| Effet médian sur OOD-1 | 3,2 % | 13,5 % | 19,3 % |
| Succès (≥ 10 %) sur OOD-1 | 7/24 | 13/24 | 18/24 |
| Succès (≥ 30 %) sur OOD-1 | 0/24 | 4/24 | 4/24 |
| Effet médian sur OOD-2 | 1,2 % | 4,9 % | 4,7 % |
| Succès (≥ 10 %) sur OOD-2 | 7/24 | 8/24 | 8/24 |
| Succès (≥ 30 %) sur OOD-2 | 1/24 | 2/24 | 0/24 |
| Champions invalides | 0 | 0 | 0 |
| Appels par run | 85 | 85 | 170 |
| Tokens en entrée | 1,51 million | 2,66 millions | 3,01 millions |
| Tokens en sortie | 1,17 million | 1,42 million | 2,34 millions |
| Coût estimé | 2,13 $ | 2,80 $ | 4,26 $ |

Coût estimé au tarif de `gemini-3.1-flash-lite` retenu par le script : 0,25 $ par million
de tokens en entrée, 1,50 $ en sortie. Total des trois bras : 9,19 $.

Champions de l'évolution qui sont des propositions de départ non modifiées : 3 sur 24
(seeds 6187, 6357, 6374).

## Excès de Best-Fit supprimé sur la validation, par seed

| Seed | Tirage 85 | Évolution | Tirage 170 |
|---|---|---|---|
| 6000 | 16,3 % | 37,0 % | 14,1 % |
| 6017 | 7,0 % | −2,1 % | 8,0 % |
| 6034 | 1,7 % | 57,2 % | 5,8 % |
| 6051 | 0,5 % | 0,5 % | 15,1 % |
| 6068 | 21,6 % | 60,0 % | 5,9 % |
| 6085 | 5,6 % | 2,0 % | 17,8 % |
| 6102 | 0,5 % | 7,7 % | 3,3 % |
| 6119 | 0,0 % | 16,7 % | 22,8 % |
| 6136 | 0,0 % | −3,3 % | 12,6 % |
| 6153 | 13,2 % | 44,4 % | 20,6 % |
| 6170 | 0,0 % | 21,2 % | 15,9 % |
| 6187 | 3,2 % | −1,1 % | 2,2 % |
| 6204 | −0,5 % | 15,6 % | 10,2 % |
| 6221 | 17,1 % | 52,9 % | 10,2 % |
| 6238 | 3,7 % | 30,7 % | 9,0 % |
| 6255 | 2,2 % | 0,0 % | 2,2 % |
| 6272 | 1,1 % | 14,6 % | 31,9 % |
| 6289 | 9,1 % | 5,1 % | 11,2 % |
| 6306 | 39,4 % | 0,0 % | 16,5 % |
| 6323 | 0,0 % | 14,8 % | 2,8 % |
| 6340 | 8,2 % | 45,4 % | 0,5 % |
| 6357 | 7,0 % | 2,1 % | 14,4 % |
| 6374 | 11,9 % | 34,2 % | 20,7 % |
| 6391 | 9,4 % | 15,7 % | −1,6 % |

## Observations

1. **Le rapport de coût mesuré n'est pas celui qui avait été supposé.** Le protocole avait
   doublé le budget du tirage parce que l'évolution consommait 2,08 fois plus de tokens sur
   MiniMax. Sur Gemini, l'évolution a coûté 1,31 fois le tirage à 85 appels (2,80 $ contre
   2,13 $) : ses prompts sont plus longs, mais l'entrée est six fois moins chère que la
   sortie. Une dépense égale aurait correspondu à environ 111 propositions de tirage, pas
   170. Aucun bras n'a été exécuté à ce budget.
2. **L'évolution a deux régimes.** Elle dépasse 30 % sur 8 seeds, contre 1 pour chacun des
   deux tirages, y compris celui qui a deux fois plus d'appels. Mais elle reste sous 10 %
   sur 10 seeds, dont 6 à 0 % ou en dessous. Doubler le tirage a l'effet inverse : il
   relève le bas (14 seeds à 10 % ou plus, comme l'évolution) sans produire de fortes
   valeurs.
3. **L'avantage de l'évolution ne se retrouve pas sur OOD-2.** Les médianes y sont proches
   (4,9 % contre 4,7 % pour le tirage 170) et l'évolution y a les plus mauvais champions :
   6 seeds sous −10 %, jusqu'à −66,7 % (seed 6374) et −45,2 % (seed 6238), contre aucune
   pour les deux tirages. Sur OOD-1, le tirage 170 a la meilleure médiane (19,3 %).
   Ces mesures sont descriptives ; elles suggèrent que les champions de l'évolution sont
   plus ajustés aux instances d'entraînement.
4. **Un effondrement dans le tirage 85** : le champion de la seed 6306, le meilleur du bras
   sur la validation (39,4 %), reste valide sur OOD-1 mais y utilise 17 271 bacs pour un
   minimum théorique de 10 068 (−1 611 %). La cause n'a pas été recherchée.
5. **Sur 3 seeds, les 63 appels après le départ n'ont rien amélioré** (champion non
   modifié), comme dans le protocole MiniMax v2 (3 sur 12).

## Limites

Celles du protocole restent entières : un seul modèle, un seul problème, un seul budget ;
réponses non reproductibles faute de champ `seed` ; les trois modifications du moteur sont
testées ensemble. Ce résultat ne dit rien des budgets plus grands, ni d'un autre problème.

## Lecture d'ensemble

À 85 appels, avec l'interpréteur corrigé et 24 seeds, l'évolution fait mieux qu'un tirage
du même nombre de propositions : c'est établi au seuil fixé à l'avance. Elle n'est pas
distinguée d'un tirage deux fois plus long, qui coûte une fois et demie son prix. La
question à dépense strictement égale reste ouverte, de même que celle qui compte pour la
grille : l'écart grandit-il quand le budget augmente ?
