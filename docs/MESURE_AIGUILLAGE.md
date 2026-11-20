# Mesure hors ligne : croisement par aiguillage de régime contre greffe AST

Mesure du 16 novembre 2026, hors protocole, sans appel au modèle. Données :
`mesures/dispatch_probe.jsonl` (aiguillage) et `mesures/crossover_probe_v3.jsonl` (greffe
corrigée). Les deux portent sur les élites des runs Gemini du protocole de courbe (seeds
8000 à 8187, entraînement), avec les mêmes paires hôte/donneur, dans deux conditions :
donneur de la même archive (`intra`) ou d'une autre seed (`inter`).

## Principe de l'aiguillage

L'enfant contient les deux programmes : `if condition { donneur } else { hôte }`. Trois
conditions testées : seconde moitié de la séquence (`late_half`), grand objet
(`large_item`), bac au-dessus du résidu moyen (`bin_above_mean`).

## Résultats

| | Enfants | Valides | Battent l'hôte | Battent les deux parents | Gain sur le meilleur parent |
|---|---|---|---|---|---|
| Aiguillage, intra | 144 | 144 | 7 | 1 | 1 bac |
| Aiguillage, inter | 144 | 144 | 58 | 13 | 1 à 9 bacs |
| Greffe, intra | 96 | 96 | 9 | — | 1 à 60 bacs sur l'hôte |
| Greffe, inter | 96 | 95 | 13 | — | 1 à 58 bacs sur l'hôte |

En `inter`, la plupart des enfants d'aiguillage qui battent l'hôte ne font que se
rapprocher d'un donneur meilleur que lui : seuls 13 sur 288 battent les deux parents.
Sur des programmes d'environ 4 100 bacs, le gain est au plus de 0,2 %. L'aiguillage
exécute les deux corps : le fuel de l'enfant monte jusqu'à près du double de celui de
l'hôte.

La greffe produit moins d'enfants améliorants, mais certains gagnent des dizaines de bacs,
et 6 battent le champion de leur run, tous confirmés en validation.

Le banc n'a pas enregistré les notes de validation des enfants d'aiguillage (champs
`val_*` vides) ; vu la taille des gains sur l'entraînement, ce manque ne change pas la
conclusion.

## Conclusion

L'aiguillage est toujours valide mais n'apporte presque rien : il combine des
comportements sans en créer de nouveau, pour un coût d'exécution doublé. Il n'est pas
intégré au moteur. La greffe reste l'opérateur testé par
`docs/PROTOCOLE_GLM_LOCAL.md` (branche weibull-benchmark).
