# Résultats du témoin « tirage sans évolution »

Témoin exécuté le 12 novembre 2026, de 19 h 28 à 0 h 04, selon
[PROTOCOLE_WEIBULL_TEMOIN.md](PROTOCOLE_WEIBULL_TEMOIN.md). Les 12 seeds sont complètes,
sans interruption. Chaque run a consommé 85 appels au modèle, comme les runs de la v2.
Données brutes : `results_weibull_temoin/`.

Pour reproduire les tableaux :

```
python scripts/analysis/temoin_vs_evolution.py
```

## Verdict

**À ce budget, le tirage sans évolution fait mieux que l'évolution.** Le témoin réussit
sur 7 seeds sur 12, au-dessus du seuil de 3 fixé par le protocole. La découverte est donc
attribuée au modèle, pas à NOMAD.

| Sur la validation, 12 seeds | Tirage | Groupe C | Groupe D |
|---|---|---|---|
| Seeds avec au moins 10 % de l'excès de Best-Fit supprimé | **7** | 4 | 4 |
| Effet médian | **19,3 %** | 2,9 % | 1,6 % |

Le tirage dépasse le seuil d'effet médian de 10 % que les campagnes v1 et v2 n'ont pas
atteint. Ce témoin n'était pas une campagne confirmatoire sur ce critère : le résultat
est descriptif.

## Excès de Best-Fit supprimé, par seed

| Seed | Validation : tirage | C | D | OOD-1 : tirage | C | D | OOD-2 : tirage | C | D |
|---|---|---|---|---|---|---|---|---|---|
| 2000 | 36,6 % | −0,5 % | 0,5 % | 64,5 % | −1,0 % | 1,0 % | −4,4 % | −4,4 % | −1,1 % |
| 2017 | 15,7 % | 0,0 % | 1,7 % | 17,5 % | 0,0 % | 1,2 % | −23,5 % | −2,4 % | −12,9 % |
| 2034 | 31,9 % | 34,6 % | 10,3 % | 67,9 % | 52,6 % | 7,8 % | 14,9 % | 36,8 % | 18,4 % |
| 2051 | 29,9 % | −1,6 % | 1,6 % | 8,6 % | 0,2 % | 3,1 % | 30,8 % | −9,9 % | 2,2 % |
| 2068 | 1,6 % | 1,1 % | 18,5 % | 1,4 % | 3,1 % | 4,3 % | 1,1 % | −11,5 % | −3,4 % |
| 2085 | 4,7 % | 31,2 % | 1,0 % | 8,4 % | 36,9 % | 2,2 % | 3,3 % | 5,6 % | −16,7 % |
| 2102 | 25,3 % | 6,6 % | 0,0 % | −0,2 % | 3,2 % | 0,0 % | 16,3 % | 10,2 % | 0,0 % |
| 2119 | 8,8 % | 4,6 % | 1,5 % | 3,6 % | 4,1 % | 0,5 % | −3,4 % | −1,1 % | 1,1 % |
| 2136 | 7,8 % | −0,5 % | 22,8 % | 8,6 % | 4,4 % | 5,7 % | 3,5 % | −7,1 % | −5,9 % |
| 2153 | 22,8 % | 0,0 % | 23,9 % | 20,2 % | −1,0 % | 81,8 % | −7,5 % | −6,2 % | 0,0 % |
| 2170 | 31,5 % | 28,4 % | 3,0 % | 36,1 % | −8,5 % | 4,2 % | 14,8 % | −123,5 % | −1,2 % |
| 2187 | 2,7 % | 39,6 % | −5,5 % | 2,6 % | −1 019,1 % | invalide | 0,0 % | 16,7 % | −5,6 % |
| **Médiane** | **19,3 %** | 2,9 % | 1,6 % | **8,6 %** | 1,7 % | 3,1 % | **2,2 %** | −3,4 % | −1,2 % |

## Observations

1. **Le tirage réussit plus souvent.** 7 seeds sur 12 contre 4 pour chaque groupe évolutif.
   Avec 12 seeds, cet écart n'est pas statistiquement établi.
2. **Le tirage généralise au moins aussi bien.** Aucun champion invalide, aucun
   effondrement sur OOD-1, et une médiane positive sur OOD-2 là où C et D sont négatifs.
   Sa plus mauvaise valeur est −23,5 % sur OOD-2 (seed 2017).
3. **Le taux de découverte par proposition fraîche est plus élevé que l'estimation
   préalable** (environ 0,5 %, tirée de 2 succès sur 336 propositions). 7 runs gagnants
   sur 12 avec 85 propositions correspondent à un taux d'environ 1 %.
4. **Les gains du tirage à l'entraînement tiennent sur la validation** : 7 seeds à plus
   de 10 % à l'entraînement, les 7 mêmes sur la validation.

## Ce que ce résultat ne dit pas

- Il ne dit pas pourquoi l'évolution fait moins bien. Hypothèse non vérifiée : une
  mutation part d'un parent proche de Best-Fit et produit une variation locale, alors
  qu'une proposition fraîche explore plus largement.
- La génération initiale reçoit l'indice de la proposition (0 à 84). Si le prompt varie
  avec cet indice, une population de 85 explore plus de variantes qu'une population de 8.
  Ce point n'a pas été vérifié.
- Il ne dit rien de budgets plus grands. L'évolution améliore souvent une découverte déjà
  faite (v1, seed 1085 : 24 % puis 49 %), ce que le tirage ne peut pas faire.
