# Protocole de la campagne Weibull (fixé avant exécution)

Rédigé le 11 novembre 2026, avant le lancement de la campagne. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Sur un benchmark où une marge existe, NOMAD produit-il des heuristiques meilleures que
Best-Fit sur des données qu'il n'a jamais vues ?

## Benchmark

Bin packing en ligne, capacité 100, tailles Weibull(forme 3, échelle 45) arrondies et
bornées à [1, 100]. Profil activé par `NOMAD_BP_PROFILE=weibull`.

| Jeu | Contenu | Rôle |
|---|---|---|
| Entraînement | 10 instances × 1 000 objets | Seul jeu vu par la sélection |
| Validation | 10 × 1 000, même loi, autre graine | **Critère principal** |
| OOD-1 | 5 × 5 000, même loi | Généralisation en longueur |
| OOD-2 | 10 × 1 000, Weibull(2, 40) | Généralisation à une autre loi |

Les jeux dépendent de la seed du run : chaque seed a ses propres données.

Différence d'interface avec les campagnes précédentes : le bac vide est un candidat noté
par `score_bin` (`bin_residual == bin_capacity`). L'heuristique peut donc ouvrir un bac
même si un bac ouvert convient. Best-Fit, lui, n'ouvre un bac que par nécessité.

## Mesures préalables (hors campagne)

- Best-Fit laisse environ 4,5 % d'excès sur la borne L1.
- Une heuristique de référence écrite à la main (dix lignes, signature inchangée) descend
  à environ 1,4 % d'excès : la marge existe et elle est atteignable.
- `qwen2.5-coder:7b` n'a rien trouvé sur la seed 1000 (champion égal à Best-Fit).
  `gemma3:27b` et `gpt-oss:20b` ont battu Best-Fit sur cette seed.

La seed 1000 a servi à ces essais et au choix du modèle. Elle est une **seed de mise au
point** : les résultats sont rapportés avec et sans elle.

## Paramètres de la campagne

- Modèle : `gpt-oss:20b` (Ollama local), choisi sur la seed 1000 uniquement.
- 9 seeds appariées (1000, 1017, ..., 1136), population 8, 6 générations.
- Groupe C (croisement + mutation) et groupe D (mutation seule) sur chaque seed.
- `NOMAD_FUEL_LIMIT=400000`.
- Code figé par le tag git `campaign-weibull-v1`.

## Critères, fixés à l'avance

**Critère principal (H-A).** Sur le jeu de validation, le champion du groupe C utilise
moins de bacs que Best-Fit. Test : Wilcoxon signé apparié, unilatéral, sur les 8 seeds
hors mise au point. Seuil : p < 0,05.

**Critères secondaires**, avec correction de Holm sur les trois :

- H-B : même comparaison sur OOD-1.
- H-C : même comparaison sur OOD-2.
- H-D : groupe C contre groupe D sur la validation (bilatéral).

**Mesure d'effet rapportée dans tous les cas** : part de l'excès de Best-Fit supprimée,
`(BF − champion) / (BF − L1)`, par seed, avec médiane et étendue. La référence manuelle
est donnée à titre de repère haut.

## Ce qui compterait comme un échec

- H-A non significative, ou effet médian inférieur à 10 % de l'excès de Best-Fit.
- Un gain à l'entraînement qui disparaît sur la validation.

## Limites connues

- 8 seeds confirmatoires : la puissance est faible pour H-D (C contre D).
- Un seul modèle. Le résultat ne dit rien des modèles plus petits, sauf l'essai négatif
  sur `qwen2.5-coder:7b`.
- Best-Fit est la baseline classique standard, mais elle ne dispose pas de l'ouverture de
  bac volontaire. La référence manuelle montre ce que cette capacité permet à elle seule.
- Le groupe C consomme plus d'évaluations que le groupe D (sondes anti-dead-code).
