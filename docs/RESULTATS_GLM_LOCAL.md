# Résultats du protocole : croisement AST local et élagage, sur glm-5.3-flash

Runs exécutés le 16 novembre 2026 de 9 h 15 à 15 h 45, selon
[PROTOCOLE_GLM_LOCAL.md](PROTOCOLE_GLM_LOCAL.md), avec l'exécutable du tag `glm-local-v2`
(SHA-256 `4606368…9044`). Les 24 runs prévus (2 bras de 12 seeds, 9600 à 9787) sont
complets, sans échec ni nouvel essai, 344 appels au modèle chacun. Le bras local a fini à
15 h 16, le témoin à 15 h 45. Données brutes : `results_glm_local/`, commitées avant le
dépouillement (commit `605238d`). Le script a d'abord été passé en mode `controle`
(0 problème).

Pour reproduire :

```
python scripts/analysis/glm_local.py
```

## Verdict

**H n'est pas établie.** À 344 appels, le bras local gagne 7 seeds et en perd 5.
Wilcoxon signé apparié, bilatéral, exact : W+ = 43, n = 12, p = 0,77. Différence médiane :
+3,9 points. La condition d'extension (p < 0,20) n'est pas remplie : un seul regard, pas de
seeds supplémentaires.

Lecture prévue par le protocole : pas d'amélioration démontrée à ce budget. Le croisement
et l'élagage locaux ne rendent pas l'évolution meilleure à appels égaux sur ce modèle.

## Effet médian par palier (validation)

Part de l'excès de Best-Fit supprimée, médiane des 12 seeds.

| Appels | 85 | 170 | 344 |
|---|---|---|---|
| Local | 40,2 % | 56,0 % | 63,3 % |
| Témoin | 36,5 % | 58,2 % | 61,3 % |
| Écart médian par seed | +4,5 | −5,9 | +3,9 |
| Seeds gagnées / perdues par le bras local | 8 / 3 | 5 / 7 | 7 / 5 |

Aucun champion invalide sur la validation, à aucun palier. Sur l'entraînement, les deux
bras finissent à la même médiane (66,6 %).

## Par seed, à 344 appels (validation, local / témoin)

| Seed | 9600 | 9617 | 9634 | 9651 | 9668 | 9685 | 9702 | 9719 | 9736 | 9753 | 9770 | 9787 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Local | 29,4 | 72,7 | 54,8 | 64,4 | 57,4 | 69,8 | 78,7 | 62,2 | 59,0 | 68,4 | 32,2 | 72,0 |
| Témoin | 86,1 | 39,6 | 59,0 | 71,3 | 38,3 | 64,2 | 76,6 | 43,1 | 68,3 | 62,0 | 60,6 | 59,7 |

Les écarts sont grands dans les deux sens (de −56,7 à +33,1 points) : la variance entre
seeds domine l'effet des opérateurs.

## Mesures descriptives, sans valeur de test

- **Excès sur la borne L1** (validation, médiane, en % de L1 ; mesure ajoutée après le
  dépouillement, sur la suggestion d'Argon, pour comparaison avec la littérature) :
  Best-Fit 4,65 %. À 344 appels : témoin 1,77 %, local 1,72 % (à 85 appels : 2,96 % et
  2,75 % ; à 170 : 1,91 % et 2,02 %).

- **Coût par appel divisé par deux.** Témoin : 0,000344 $ par appel (comme l'évolution du
  protocole GLM à 344 appels, 0,000345 $). Local : 0,000170 $. Rapport local / témoin :
  **0,494**. Tokens en entrée : 15,1 contre 7,8 millions ; en sortie : 10,4 contre 5,0
  millions. Coût total : témoin 1,42 $, local 0,70 $.
- **Code trois fois plus court.** Taille médiane du champion final : 12 280 caractères
  dans le témoin, 3 918 dans le bras local.
- **Les opérateurs locaux produisent des champions.** Champion final du bras local :
  6 mutations, 3 croisements, 3 élagages. Champions successifs : 155 mutations,
  65 élagages, 45 croisements, 35 propositions fraîches.
- **Compteurs.** 1 656 croisements tentés, 0 erreur, 239 acceptés (14 %) ; 135 élagages
  tentés, 82 retenus, 211 instructions retirées ; 3 594 évaluations locales en tout.

## Observations

1. **Pas de gain de qualité à appels égaux.** Les médianes finales sont proches (63,3 %
   contre 61,3 %) et le test est loin du seuil. Les opérateurs locaux trouvent des
   champions, mais le témoin y arrive aussi par mutation.
2. **Le gain est sur le coût.** Avec l'élagage, les programmes restent trois fois plus
   courts, et un appel d'évolution coûte deux fois moins cher. C'était la cause de l'échec
   de H2 sur GLM ([RESULTATS_GLM_340.md](RESULTATS_GLM_340.md)) : un appel d'évolution y
   coûtait 4,36 fois un appel de tirage. Avec le prix par appel du tirage mesuré alors
   (0,000079 $), le rapport tomberait vers 2,2, et l'évolution à dépense égale serait
   arrêtée vers 160 appels au lieu de 78. C'est une estimation, sur un autre exécutable et
   d'autres seeds : seul un protocole peut le tester.
3. **Le bras local n'est pas plus fragile.** Aucun champion invalide, et les deux bras
   finissent au même niveau sur l'entraînement.
4. **Seed 9600** : le bras local reste bloqué à 29,4 % dès 170 appels, quand le témoin
   monte à 86,1 %. C'est la plus grosse perte ; elle pèse sur la médiane des écarts mais
   pas sur le signe du verdict.

## Limites

Celles du protocole : 12 seeds, un seul problème, un seul modèle servi en fp4 ; les deux
opérateurs sont testés ensemble ; le bras local fait plus de calcul local, non facturé ;
réponses du modèle non reproductibles.

## Ce que ce résultat suggère pour la suite

Hors protocole, à décider :

- **Retester H2 sur GLM avec l'élagage** : évolution avec opérateurs locaux contre tirage, à
  dépense égale, avec le rapport de coût mesuré dans le protocole. C'est là que le gain
  observé ici peut compter.
- **Séparer les deux opérateurs** : l'élagage seul explique sans doute la baisse du coût ;
  le croisement local n'a pas d'effet démontré.
