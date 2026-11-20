# Résultats : courbe de progression de l'évolution à l'intérieur des runs

Calcul fait le 14 novembre 2026 selon
[PROTOCOLE_COURBE_ECHELLE.md](PROTOCOLE_COURBE_ECHELLE.md), sans nouveau run. Toutes les
mesures portent sur l'entraînement.

Pour reproduire :

```
python scripts/analysis/gemini_courbe_echelle.py
```

## Verdict

**L'évolution monte encore à 340 appels, au sens du critère fixé à l'avance, mais tout
juste** : 12 seeds sur 24 progressent dans la dernière tranche, et le seuil était 12.
La lecture prévue est qu'un troisième budget est justifié.

## M1 : progression par tranche de 84 appels (évolution à 340 appels, 24 seeds)

| Tranche | Seeds qui progressent | Gain médian | Gain moyen |
|---|---|---|---|
| De 88 à 172 appels | 23/24 | 9,7 points | 16,5 points |
| De 172 à 256 appels | 18/24 | 3,5 points | 7,6 points |
| De 256 à 340 appels | 12/24 | 0,3 point | 6,7 points |

## M2 : effet médian à l'entraînement selon le nombre d'appels

| Appels | Tirage | Évolution (seeds 7000 à 7391) |
|---|---|---|
| 22 | 1,1 % | |
| 85 | 4,3 % | |
| 88 | 3,5 % | 3,5 % (départ : c'est un tirage) |
| 170 ou 172 | 10,3 % | 15,8 % |
| 256 | | 22,3 % |
| 340 | 16,9 % | 35,9 % |
| 455 | 16,9 % | |

Les points du tirage viennent de runs et de seeds différents ; ce tableau est descriptif.

## Observations

1. **Les progrès se raréfient mais ne s'arrêtent pas.** Le nombre de seeds qui progressent
   baisse d'une tranche à l'autre (23, 18, 12) et le gain médian tombe à 0,3 point. Le
   gain moyen, lui, reste élevé (6,7 points) : dans la dernière tranche, la moitié des
   seeds ne bouge pas et quelques-unes font un bond (seed 7357 : de 14,6 % à 64,9 % ;
   7272 : de 8,0 % à 50,5 % ; 7170 : de 14,1 % à 42,7 %).
2. **L'évolution avance par sauts.** Un champion reste longtemps inchangé puis s'améliore
   d'un coup. Hors protocole : le dernier appel ayant amélioré le champion se situe, en
   médiane, vers le 260e appel sur 340 ; pour 7 seeds sur 24 il est après le 295e.
3. **Le tirage ralentit plus tôt.** Sa médiane ne bouge pas entre 340 et 455 propositions
   (16,9 %), alors que celle de l'évolution passe de 22,3 % à 35,9 % entre 256 et
   340 appels.
4. **Le départ large semble coûteux** (hors protocole, seeds différentes). Avec 88
   propositions de départ, l'évolution atteint 15,8 % au 172e appel. Dans le protocole de
   confirmation, avec 22 propositions de départ, elle atteignait 15,3 % au 85e appel. Les
   66 propositions de départ supplémentaires n'ont donc presque rien apporté : un départ
   plus court, suivi de plus de générations, mériterait d'être essayé.

## Limites

Mesures à l'entraînement, sans valeur pour la généralisation. Le verdict tient à une seed
près. Un run de 340 appels arrêté en route n'est pas un run conçu pour un budget plus
court. Cette analyse ne teste pas l'effet d'échelle.

## Pour la suite

Les rapports ne conservent pas le score de chaque proposition, ce qui a empêché la
comparaison appariée avec le tirage aux budgets intermédiaires. Enregistrer ce score dans
les rapports rendrait cette comparaison possible pour tout run futur, sans dépense
supplémentaire.
