# Résultats de la campagne Weibull v2

Campagne exécutée du 11 novembre 2026, 20 h 19, au 12 novembre, 11 h 57, selon
[PROTOCOLE_WEIBULL_V2.md](PROTOCOLE_WEIBULL_V2.md). Les 12 paires de runs prévues sont
complètes. Données brutes : `results_weibull_v2/`.

Pour reproduire les tableaux et les tests :

```
python scripts/analysis/weibull_vs_baseline.py results_weibull_v2 target/frozen/nomad-campaign-weibull-v2.exe
```

## Verdict

**La campagne est un échec au sens du protocole**, pour la même raison que la v1 : le
test principal est significatif, mais l'effet médian reste sous le seuil de 10 %.

| Critère | Résultat | Seuil du protocole | Verdict |
|---|---|---|---|
| H-A, test : C < Best-Fit sur validation | 7 seeds gagnées sur 12, p = 0,018 | p < 0,05 | Atteint |
| H-A, effet médian (excès de Best-Fit supprimé) | 2,9 % (de −1,6 % à 39,6 %) | ≥ 10 % | **Non atteint** |
| H-B : C < Best-Fit sur OOD-1 | 7/12, p = 0,23 (Holm 0,68) | p < 0,05 | Non atteint |
| H-C : C < Best-Fit sur OOD-2 | 4/12, p = 0,65 (Holm 0,82) | p < 0,05 | Non atteint |
| H-D : C contre D sur validation | C 6, D 6, p = 0,41 (Holm 0,82) | p < 0,05 | Non atteint |

Le second critère d'échec (un gain à l'entraînement qui disparaît sur la validation) n'est
pas observé : les quatre seeds où C gagne nettement à l'entraînement gagnent aussi sur la
validation.

## Excès de Best-Fit supprimé sur la validation, par seed

| Seed | Groupe C | Groupe D |
|---|---|---|
| 2000 | −0,5 % | 0,5 % |
| 2017 | 0,0 % | 1,7 % |
| 2034 | 34,6 % | 10,3 % |
| 2051 | −1,6 % | 1,6 % |
| 2068 | 1,1 % | 18,5 % |
| 2085 | 31,2 % | 1,0 % |
| 2102 | 6,6 % | 0,0 % |
| 2119 | 4,6 % | 1,5 % |
| 2136 | −0,5 % | 22,8 % |
| 2153 | 0,0 % | 23,9 % |
| 2170 | 28,4 % | 3,0 % |
| 2187 | 39,6 % | −5,5 % |

## Observations

1. **Doubler les générations n'a pas augmenté la part de seeds gagnantes.** Le groupe C
   dépasse 10 % d'effet sur la validation sur 4 seeds sur 12 (2034, 2085, 2170, 2187),
   contre 4 sur 9 en v1. L'hypothèse du protocole v2, un budget de recherche trop court,
   n'est pas confirmée.
2. **Le croisement n'apporte rien de mesurable.** C et D gagnent chacun 6 seeds. Le
   groupe D dépasse lui aussi 10 % d'effet sur 4 seeds sur 12 (2034, 2068, 2136, 2153).
   Une découverte nette survient dans environ un run sur trois, quel que soit le groupe.
3. **Les gains ne se transfèrent pas à l'autre loi (OOD-2).** L'effet médian de C y est
   négatif (−3,4 %) et C y fait moins bien que Best-Fit sur 8 seeds sur 12. Sur la seed
   2170, le champion de C gagne 28 % sur la validation et perd 124 % sur OOD-2.
4. **Deux échecs réels sur les longues instances (OOD-1), seed 2187.** Le champion de C,
   qui a le meilleur effet sur la validation (39,6 %), utilise 14 799 bacs contre 10 478
   pour Best-Fit. Le champion de D est invalide, y compris avec une limite de fuel dix
   fois plus haute : ce n'est pas un effet de la limite. La correction du fuel a par
   ailleurs fonctionné : aucun autre champion n'est invalide.
5. **La référence manuelle reste devant** sur l'entraînement et la validation pour toutes
   les seeds. Elle est battue deux fois hors distribution : sur OOD-1 par le champion de D
   de la seed 2153 (10 117 bacs contre 10 157) et sur OOD-2 par le champion de C de la
   seed 2034 (3 592 contre 3 608, le champion de D faisant jeu égal).

## Lecture d'ensemble des deux campagnes

Sur 21 seeds et deux budgets de recherche, le résultat est stable : NOMAD trouve une
heuristique nettement meilleure que Best-Fit sur la validation dans environ un run sur
trois, et rien de notable dans les autres. Ni le doublement des générations ni le
croisement ne changent cette proportion. Les heuristiques trouvées tiennent sur des
données de même loi, moins bien sur des instances plus longues, et pas sur une autre loi.

## Écarts par rapport au protocole

- **Interruption et reprise.** Le PC a redémarré à 0 h 10 le 12 novembre sur une erreur
  système Windows (vérification d'erreur 0x3B), 27 secondes après le début de la seed
  2085. Les seeds 2000 à 2068 étaient complètes. La campagne a repris à 6 h 42 avec le
  même exécutable et les mêmes paramètres (`--seed-start 2085 --seeds 7`). Aucune seed
  n'a été rejouée après lecture de son résultat. Les journaux et l'agrégat de la première
  partie sont conservés sous les noms `campaign_part1.*` et
  `campaign_aggregate_part1.json`. Le fichier `campaign_aggregate.json` ne contient que
  les 7 dernières seeds.
- **Point d'étape.** Les résultats des 5 premières seeds ont été lus pendant
  l'interruption. Aucun paramètre n'a été modifié ensuite.
- **Durée.** 47 minutes par seed en moyenne, contre 73 estimées.
