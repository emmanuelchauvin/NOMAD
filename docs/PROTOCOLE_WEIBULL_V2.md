# Protocole de la campagne Weibull v2 (fixé avant exécution)

Rédigé le 11 novembre 2026, après la campagne v1 et avant le lancement de la v2. Les
critères ci-dessous ne doivent pas être modifiés après lecture des résultats.

La campagne v1 ([résultats](RESULTATS_WEIBULL_V1.md)) est un échec au sens de son
protocole. Elle sert ici de pilote : elle justifie les deux changements ci-dessous et rien
d'autre. La v2 est une nouvelle campagne confirmatoire, sur des seeds jamais utilisées.

## Question

Inchangée : sur un benchmark où une marge existe, NOMAD produit-il des heuristiques
meilleures que Best-Fit sur des données qu'il n'a jamais vues ?

## Ce qui change par rapport à la v1

| Paramètre | v1 | v2 | Raison |
|---|---|---|---|
| Limite de fuel par décision | 400 000, fixe | 400 000 × ⌈objets / 1 000⌉ | Voir « Fuel » |
| Générations | 6 | 12 | Voir « Budget de recherche » |
| Seeds | 9 (1000 à 1136), dont 1 de mise au point | 12 (2000 à 2187), toutes confirmatoires | Seeds neuves |

Tout le reste est identique à la v1 : benchmark, jeux de données, modèle, population,
groupes, hypothèses, tests et seuils.

### Fuel

En v1, cinq champions (trois du groupe C, deux du groupe D) étaient invalides sur OOD-1.
Vérification faite après la campagne : les cinq échouent à 400 000 de fuel par décision
et passent à 4 000 000, sans autre changement. Une décision note chaque bac ouvert, donc
son coût croît avec la longueur de l'instance. Une limite fixe, calibrée sur 1 000 objets,
élimine sur 5 000 objets des heuristiques valides.

La limite devient proportionnelle à la longueur de l'instance : 400 000 jusqu'à
1 000 objets, 2 000 000 sur OOD-1. Les scores sur l'entraînement, la validation et OOD-2
ne sont pas affectés. Best-Fit et la référence manuelle sont évalués avec la même règle.

### Budget de recherche

En v1, chaque run consommait 43 appels au modèle. Sur les trois seeds où le groupe C ne
gagne rien (1102, 1119, 1136), le champion est une heuristique de la population initiale
et aucune génération ne l'a améliorée. Sur les seeds gagnantes, le saut décisif apparaît
tard (génération 4 sur 1000 et 1017) et deux runs progressent encore à la génération 6
(1034, 1085). Le budget de 6 générations est donc trop court pour que la recherche
aboutisse sur une majorité de seeds.

La v2 passe à 12 générations, soit environ 85 appels par run. Ce choix est fait sur les
données de la v1 et il est figé ici.

## Benchmark

Bin packing en ligne, capacité 100, tailles Weibull(forme 3, échelle 45) arrondies et
bornées à [1, 100]. Profil activé par `NOMAD_BP_PROFILE=weibull`. Le bac vide est un
candidat noté par `score_bin`.

| Jeu | Contenu | Rôle |
|---|---|---|
| Entraînement | 10 instances × 1 000 objets | Seul jeu vu par la sélection |
| Validation | 10 × 1 000, même loi, autre graine | **Critère principal** |
| OOD-1 | 5 × 5 000, même loi | Généralisation en longueur |
| OOD-2 | 10 × 1 000, Weibull(2, 40) | Généralisation à une autre loi |

Les jeux dépendent de la seed du run : chaque seed a ses propres données.

## Paramètres de la campagne

- Modèle : `gpt-oss:20b` (Ollama 0.40.16, local).
- 12 seeds appariées : 2000, 2017, ..., 2187 (`--seed-start 2000 --seeds 12`). Aucune n'a
  servi à un essai avec le modèle. Les seeds 2000 et 2017 ont servi à un essai à blanc de
  la commande avec l'opérateur factice (sans modèle, 2 générations), dont aucun paramètre
  n'a été tiré.
- Population 8, 12 générations.
- Groupe C (croisement + mutation) et groupe D (mutation seule) sur chaque seed.
- `NOMAD_FUEL_LIMIT=400000`, mis à l'échelle comme décrit plus haut.
- Code figé par le tag git `campaign-weibull-v2`, exécutable
  `target/frozen/nomad-campaign-weibull-v2.exe`.

Commande de lancement :

```
$env:NOMAD_BP_PROFILE = 'weibull'; $env:NOMAD_FUEL_LIMIT = '400000'
target\frozen\nomad-campaign-weibull-v2.exe campaign --domain bin-packing --seeds 12 --seed-start 2000 --pop-size 8 -n 12 --operator ollama --model gpt-oss:20b --output-dir results_weibull_v2
```

Analyse :

```
python scripts/analysis/weibull_vs_baseline.py results_weibull_v2 target/frozen/nomad-campaign-weibull-v2.exe
```

## Critères, fixés à l'avance

**Critère principal (H-A).** Sur le jeu de validation, le champion du groupe C utilise
moins de bacs que Best-Fit. Test : Wilcoxon signé apparié, unilatéral, sur les 12 seeds.
Seuil : p < 0,05.

**Critères secondaires**, avec correction de Holm sur les trois :

- H-B : même comparaison sur OOD-1.
- H-C : même comparaison sur OOD-2.
- H-D : groupe C contre groupe D sur la validation (bilatéral).

Un champion invalide sur un jeu compte comme une défaite sur ce jeu.

**Mesure d'effet rapportée dans tous les cas** : part de l'excès de Best-Fit supprimée,
`(BF − champion) / (BF − L1)`, par seed, avec médiane et étendue. La référence manuelle
est donnée à titre de repère haut.

## Ce qui compterait comme un échec

- H-A non significative, ou effet médian inférieur à 10 % de l'excès de Best-Fit.
- Un gain à l'entraînement qui disparaît sur la validation.

Ces deux critères sont ceux de la v1, inchangés.

## Limites connues

- Le passage à 12 générations repose sur 9 runs pilotes. Rien ne garantit que l'effet
  médian dépasse 10 % : en v1, 4 seeds sur 9 dépassaient ce niveau sur la validation.
- 12 seeds : la puissance reste faible pour H-D (C contre D).
- Un seul modèle.
- Best-Fit ne dispose pas de l'ouverture de bac volontaire. La référence manuelle montre
  ce que cette capacité permet à elle seule.
- Le groupe C consomme plus d'évaluations que le groupe D (sondes anti-dead-code).
- La durée estimée est de 14 à 15 heures (environ 73 minutes par seed, par extrapolation
  des 37 minutes de la v1).
