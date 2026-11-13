# Résultats « tirage contre évolution corrigée » sur MiniMax

Runs exécutés le 13 novembre 2026 selon
[PROTOCOLE_MINIMAX_TIRAGE_EVOLUTION.md](PROTOCOLE_MINIMAX_TIRAGE_EVOLUTION.md) : bras
tirage de 10 h 26 à 12 h 27, bras évolution de 12 h 31 à 15 h 33. Les 24 runs prévus sont
complets, sans échec ni nouvel essai. Données brutes : `results_minimax_v1/`.

Pour reproduire les tableaux et le test :

```
python scripts/analysis/minimax_tirage_evolution.py
```

## Verdict

**Pas de différence montrée à ce budget.** Les deux bras réussissent sur 9 seeds sur 12.
Le protocole demandait un écart d'au moins 3 succès pour parler d'un indice.

| Mesure | Tirage | Évolution |
|---|---|---|
| Condition préalable (tirage ≥ 2 succès) | 9/12, remplie | |
| **Principale : succès (≥ 10 %) sur la validation** | **9/12** | **9/12** |
| Effet médian sur la validation | 21,9 % | 23,0 % |
| Succès sur OOD-1 | 8/12 | 8/12 (1 champion invalide) |
| Effet médian sur OOD-1 | 25,9 % | 27,5 % (sur 11 valides) |
| Succès sur OOD-2 | 5/12 | 8/12 |
| Effet médian sur OOD-2 | 9,4 % | 13,9 % |
| Seeds gagnées sur la validation | 6 | 6 |

Test de Wilcoxon signé apparié, bilatéral, exact, sur la validation : W+ = 48, n = 12,
p = 0,52.

## Excès de Best-Fit supprimé sur la validation, par seed

| Seed | Tirage | Évolution |
|---|---|---|
| 4000 | 21,6 % | 26,8 % |
| 4017 | 29,9 % | 19,3 % |
| 4034 | 22,1 % | 44,2 % |
| 4051 | 1,1 % | 0,0 % |
| 4068 | 29,8 % | 58,6 % |
| 4085 | 32,1 % | 13,7 % |
| 4102 | 12,5 % | 30,7 % |
| 4119 | 30,8 % | −0,5 % |
| 4136 | 23,9 % | 5,7 % |
| 4153 | 16,1 % | 10,4 % |
| 4170 | −3,8 % | 60,0 % |
| 4187 | 3,9 % | 38,3 % |

## Observations

Les observations 2 à 4 n'étaient pas des mesures du protocole. Elles ont été faites après
lecture des résultats et ne valent que comme pistes.

1. **Avec la sélection corrigée, l'évolution n'est plus derrière le tirage.** Avec
   `gpt-oss:20b` et l'ancienne sélection, elle réussissait sur 4 seeds sur 12 contre 7
   pour le tirage. Le modèle a changé en même temps que la sélection : cette comparaison
   ne permet pas d'attribuer le progrès à la correction seule.
2. **Les meilleurs champions de l'évolution vont plus haut.** Sur la validation,
   l'évolution dépasse 30 % sur 5 seeds (maximum 60,0 %), le tirage sur 2 (maximum
   32,1 %). En contrepartie, l'évolution reste à zéro ou presque sur 3 seeds (4051, 4119,
   4136) où elle ne décolle pas de sa population de départ.
3. **Le tirage dépend d'un archétype du prompt.** D'après le numéro des propositions, 9
   champions du tirage sur 12, et 8 de ses 9 succès, viennent de l'archétype « éviter les
   espaces morts », proche de l'astuce de la référence manuelle. Les champions de
   l'évolution descendent de cet archétype sur 4 seeds seulement ; 6 de ses 9 succès
   partent d'un autre archétype. Le code des champions n'a pas été relu pour le confirmer.
4. **Les champions de l'évolution sont le produit de longues lignées.** 8 champions sur
   12 comptent au moins 7 mutations successives.
5. **Un champion invalide sur OOD-1** : évolution, seed 4170, alors qu'il a le meilleur
   effet sur la validation (60,0 %). La cause n'a pas été recherchée.

## Écarts et remarques

- **Budget en tokens inégal.** Les deux bras font 85 appels par run, comme prévu, mais le
  prompt de mutation est plus long : 2,83 millions de tokens pour l'évolution contre 1,51
  pour le tirage, soit environ 1,92 dollar contre 1,05 au tarif de `MiniMax-M3`.
- **Durée.** Environ 10 minutes par seed pour le tirage, 15 pour l'évolution.
- Le test de Wilcoxon a été calculé de façon exacte par énumération, sans bibliothèque.

## Lecture d'ensemble

À 85 appels, ce benchmark est résolu 3 fois sur 4 par un simple tirage avec ce modèle. Il
ne laisse donc à l'évolution que peu de place pour montrer un avantage sur la mesure
principale. La suite demande soit un seuil de succès plus exigeant, soit un problème où
le tirage échoue, fixé dans un nouveau protocole avant tout run.
