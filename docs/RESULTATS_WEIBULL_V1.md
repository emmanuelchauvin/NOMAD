# Résultats de la campagne Weibull v1

Campagne exécutée le 11 novembre 2026, de 15 h 13 à 18 h 46, selon
[PROTOCOLE_WEIBULL.md](PROTOCOLE_WEIBULL.md). Les 9 paires de runs prévues sont complètes.
Données brutes : `results_weibull_v1/`.

Pour reproduire les tableaux et les tests :

```
python scripts/analysis/weibull_vs_baseline.py results_weibull_v1 target/frozen/nomad-campaign-weibull-v1.exe
```

## Verdict

**La campagne est un échec au sens du protocole.** Le test principal est significatif,
mais l'effet médian est sous le seuil de 10 % que le protocole fixait à l'avance.

| Critère | Résultat | Seuil du protocole | Verdict |
|---|---|---|---|
| H-A, test : C < Best-Fit sur validation | 5 seeds gagnées sur 8, p = 0,031 | p < 0,05 | Atteint |
| H-A, effet médian (excès de Best-Fit supprimé) | 3,2 % (de −1,1 % à 70,4 %) | ≥ 10 % | **Non atteint** |
| H-B : C < Best-Fit sur OOD-1 | 3/8, p = 0,84 (Holm 0,84) | p < 0,05 | Non atteint |
| H-C : C < Best-Fit sur OOD-2 | 5/8, p = 0,031 (Holm 0,094) | p < 0,05 | Non atteint |
| H-D : C contre D sur validation | C 5, D 2, égalité 1, p = 0,28 (Holm 0,56) | p < 0,05 | Non atteint |

Les tests portent sur les 8 seeds hors mise au point. Avec la seed 1000, l'effet médian
de C sur la validation est de 4,3 %, toujours sous le seuil.

Le second critère d'échec du protocole (un gain à l'entraînement qui disparaît sur la
validation) n'est pas observé : les seeds qui gagnent à l'entraînement gagnent aussi sur
la validation, avec un effet réduit sur la seed 1085 (48,9 % puis 22,3 %).

## Excès de Best-Fit supprimé par le groupe C, par seed

| Seed | Entraînement | Validation | OOD-1 | OOD-2 |
|---|---|---|---|---|
| 1000 (mise au point) | 37,3 % | 37,5 % | 39,2 % | 12,9 % |
| 1017 | 20,7 % | 19,3 % | invalide | 0,0 % |
| 1034 | 3,7 % | 4,3 % | invalide | 3,4 % |
| 1051 | 70,6 % | 70,4 % | 83,5 % | 52,8 % |
| 1068 | 4,1 % | −1,1 % | 0,9 % | 3,2 % |
| 1085 | 48,9 % | 22,3 % | invalide | 19,6 % |
| 1102 | 0,0 % | 0,0 % | 0,0 % | 0,0 % |
| 1119 | 1,6 % | 2,1 % | 1,2 % | 1,0 % |
| 1136 | 0,0 % | 0,0 % | 0,0 % | 0,0 % |

## Observations

1. **L'effet est concentré sur trois seeds.** Sur la validation, C supprime 70 % de
   l'excès sur la seed 1051, 22 % sur 1085 et 19 % sur 1017. Sur les cinq autres seeds
   confirmatoires, l'effet va de −1,1 % à 4,3 %. Sur 1102 et 1136, le champion de C fait
   exactement le score de Best-Fit sur les quatre jeux.
2. **La référence manuelle reste devant.** Seule la seed 1051 l'atteint sur la validation
   (4 089 bacs contre 4 090).
3. **Cinq champions sont invalides sur OOD-1.** Trois du groupe C (seeds 1017, 1034, 1085)
   et deux du groupe D (seeds 1051, 1085) ne résolvent aucune des 5 instances de
   5 000 objets. Le script d'analyse les compte comme des défaites, ce qui explique en
   grande partie le résultat de H-B. La cause n'est pas établie à la date de cette note.
   L'hypothèse à vérifier est un dépassement de la limite de fuel par décision quand le
   nombre de bacs ouverts augmente.
4. **Le groupe D fait souvent moins bien que Best-Fit sur OOD-2** (jusqu'à −33,7 % sur la
   seed 1051). L'écart entre C et D n'est pas démontré avec 8 seeds.

## Écarts par rapport au protocole

- L'en-tête du journal de campagne annonce une campagne « 30 seeds ». C'est un libellé du
  programme : 9 paires étaient prévues et 9 ont été exécutées.
- La valeur de `NOMAD_FUEL_LIMIT` n'est pas enregistrée dans les fichiers de résultats.
  Le protocole prévoit 400 000.
- Les essais préalables sur la seed 1000 ont tourné sur une version antérieure d'Ollama.
  La campagne entière a tourné sur Ollama 0.40.16.
