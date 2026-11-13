# Résultats du protocole MiniMax v2

Runs exécutés le 13 novembre 2026, de 16 h 45 à 18 h 22, selon
[PROTOCOLE_MINIMAX_V2.md](PROTOCOLE_MINIMAX_V2.md). Les 24 runs prévus sont complets, sans
échec ni nouvel essai. Données brutes : `results_minimax_v2/`.

Pour reproduire les tableaux et le test :

```
python scripts/analysis/minimax_tirage_evolution.py v2
```

## Verdict

**Pas de différence montrée au sens du protocole.** Sur la mesure principale, l'évolution
réussit sur 4 seeds et le tirage sur 2. Le protocole demandait un écart d'au moins 3.

Les mesures secondaires vont toutes dans le sens de l'évolution, sans être significatives.

| Mesure | Tirage | Évolution |
|---|---|---|
| **Principale : succès (≥ 30 %) sur la validation** | **2/12** | **4/12** |
| Succès (≥ 10 %) sur la validation | 5/12 | 8/12 |
| Effet médian sur la validation | 9,8 % | 21,5 % |
| Succès (≥ 10 %) sur OOD-1 | 7/12 | 9/12 |
| Effet médian sur OOD-1 | 13,4 % | 61,7 % |
| Succès (≥ 10 %) sur OOD-2 | 4/12 | 6/12 |
| Effet médian sur OOD-2 | 5,9 % | 7,8 % |
| Champions invalides | 0 | 0 |
| Seeds gagnées sur la validation | 5 | 7 |
| Tokens (entrée + sortie) | 1,50 million | 3,12 millions |
| Coût estimé | 1,04 $ | 2,01 $ |

Test de Wilcoxon signé apparié, bilatéral, exact, sur la validation : W+ = 51, n = 12,
p = 0,38.

## Excès de Best-Fit supprimé sur la validation, par seed

| Seed | Tirage | Évolution |
|---|---|---|
| 5000 | 4,3 % | 22,5 % |
| 5017 | 1,6 % | 9,4 % |
| 5034 | 39,3 % | 14,2 % |
| 5051 | 50,3 % | 8,0 % |
| 5068 | 9,8 % | 47,9 % |
| 5085 | 9,8 % | 36,3 % |
| 5102 | 8,5 % | 3,2 % |
| 5119 | 27,2 % | 26,1 % |
| 5136 | 2,6 % | 0,0 % |
| 5153 | 9,1 % | 20,5 % |
| 5170 | 10,1 % | 34,0 % |
| 5187 | 20,3 % | 36,4 % |

## Observations

1. **Le tirage varie beaucoup d'un jeu de seeds à l'autre.** Avec le même modèle et la même
   commande, il passe de 9 succès sur 12 au seuil de 10 % (seeds 4000 à 4187) à 5 sur 12
   (seeds 5000 à 5187). Douze seeds donnent donc une mesure très bruitée, pour les deux
   bras.
2. **Trois champions de l'évolution sont des propositions de départ non modifiées** (seeds
   5068, 5136, 5187). Sur ces seeds, les 63 appels qui suivent le départ n'ont rien
   amélioré.
3. **Un effondrement sur les longues instances** : le champion de l'évolution de la seed
   5136 est au niveau de Best-Fit sur la validation (0,0 %) et utilise bien plus de bacs
   que lui sur OOD-1 (−274 %). La cause n'a pas été recherchée.
4. **L'évolution consomme deux fois plus de tokens** pour le même nombre d'appels. À
   dépense égale en dollars, le tirage aurait droit à environ deux fois plus de
   propositions. Ce protocole ne dit pas ce que donnerait cette comparaison.
5. Les trois modifications du moteur ont été testées ensemble : rien ne dit laquelle
   compte, ni même si l'écart avec le protocole précédent vient d'elles ou des seeds.

## Lecture d'ensemble

Sur deux jeux de 12 seeds, l'évolution fait au moins aussi bien que le tirage au seuil de
10 % (9 contre 9, puis 8 contre 5) et mieux au seuil de 30 % (5 contre 2, puis 4 contre 2).
Le sens est constant, mais aucun de ces écarts n'atteint le seuil fixé à l'avance, et le
premier écart à 30 % a servi à choisir ce seuil. Il faut plus de seeds, et une comparaison
à dépense égale, pour savoir si l'avantage est réel.
