# Résultats du protocole de courbe d'échelle sur Gemini (1 359 appels)

## Premier regard : 12 seeds

Runs exécutés le 14 novembre 2026, de 9 h 58 à 19 h 06, selon
[PROTOCOLE_GEMINI_COURBE.md](PROTOCOLE_GEMINI_COURBE.md), avec l'exécutable figé par le tag
`gemini-courbe`. Les 24 runs prévus (2 bras de 12 seeds, 8000 à 8187) sont complets, sans
échec ni nouvel essai. Données brutes : `results_gemini_courbe/`, commitées avant le
dépouillement (commit `aa17e28`). Script de dépouillement commité avant la lecture des
scores (commit `8899240`).

Pour reproduire les tableaux et les tests :

```
python scripts/analysis/gemini_courbe.py
```

### Verdict du premier regard

**H1 et H2 sont établies à 1 359 appels. H3 ne l'est pas, de peu ; l'extension prévue par
le protocole est déclenchée.**

- **H1, à appels égaux (1 359 contre 1 359).** L'évolution gagne 11 seeds et en perd 1.
  Wilcoxon signé apparié, bilatéral, exact : W+ = 73, n = 12, p = 0,0049. Différence
  médiane : 41,4 points.
- **H2, à dépense égale.** Rapport de coût mesuré `r` = 1,5237, donc `k*` = 891 :
  l'évolution arrêtée à 891 appels contre le tirage complet de 1 359 propositions.
  L'évolution gagne 10 seeds et en perd 2. W+ = 71, n = 12, p = 0,0093. Différence
  médiane : 39,5 points.
- **H3, test principal : écart à 1 359 appels contre écart à 340 appels.** L'écart grandit
  sur 8 seeds, se réduit sur 3, est inchangé sur 1. W+ = 54, n = 11, p = 0,0674.
  Différence médiane : 6,5 points. Non établie au seuil de 0,05.
- **Extension.** p < 0,20 et l'écart grandit sur plus de seeds qu'il ne se réduit : la
  condition fixée à l'avance est remplie. Les 12 seeds supplémentaires (8204 à 8391) ont
  été lancées le 15 novembre à 2 h 10, à l'identique. Le test final de H3 portera sur les
  24 seeds, au seuil de 0,025.

### Courbes : effet médian par palier

Part de l'excès de Best-Fit supprimée, médiane des 12 seeds.

| Appels | 85 | 170 | 340 | 680 | 1 359 |
|---|---|---|---|---|---|
| Évolution, validation | 14,3 % | 24,8 % | 34,0 % | 41,2 % | 57,5 % |
| Tirage, validation | 1,1 % | 5,9 % | 14,1 % | 15,5 % | 17,1 % |
| Évolution, entraînement | 15,9 % | 29,4 % | 38,0 % | 45,3 % | 63,4 % |
| Tirage, entraînement | 3,0 % | 7,4 % | 15,5 % | 18,1 % | 21,1 % |
| Écart médian par seed (validation) | 10,0 | 21,1 | 18,9 | 25,0 | 41,4 |
| Seeds gagnées par l'évolution | 9/12 | 10/12 | 9/12 | 11/12 | 11/12 |

Succès sur la validation :

| Appels | 85 | 170 | 340 | 680 | 1 359 |
|---|---|---|---|---|---|
| Évolution ≥ 10 % | 7/12 | 9/12 | 10/12 | 11/12 | 12/12 |
| Évolution ≥ 30 % | 3/12 | 6/12 | 7/12 | 7/12 | 10/12 |
| Tirage ≥ 10 % | 2/12 | 5/12 | 8/12 | 9/12 | 11/12 |
| Tirage ≥ 30 % | 1/12 | 1/12 | 1/12 | 1/12 | 1/12 |

Aucun champion invalide sur la validation, à aucun palier.

### Excès de Best-Fit supprimé sur la validation, par seed (évolution / tirage)

| Seed | 85 | 170 | 340 | 680 | 1 359 |
|---|---|---|---|---|---|
| 8000 | 41,4 / −2,7 | 41,9 / 14,5 | 44,6 / 14,5 | 45,2 / 14,5 | 57,0 / 15,6 |
| 8017 | 15,5 / 5,0 | 15,5 / 2,8 | 13,3 / 11,0 | 18,2 / 7,7 | 50,3 / 19,3 |
| 8034 | 13,2 / 3,7 | 13,2 / 9,0 | 13,2 / 9,0 | 17,5 / 9,0 | 37,0 / 18,0 |
| 8051 | 2,7 / 0,5 | 61,7 / 0,0 | 63,8 / 17,6 | 63,8 / 17,6 | 63,8 / 17,6 |
| 8068 | −3,3 / 47,2 | −3,3 / 47,2 | 6,1 / 47,2 | 6,1 / 47,2 | 11,7 / 47,2 |
| 8085 | 44,8 / 8,3 | 44,8 / 15,5 | 64,6 / 15,5 | 64,6 / 16,6 | 58,0 / 16,6 |
| 8102 | 15,5 / 1,1 | 32,6 / 0,0 | 32,6 / 9,9 | 42,5 / 9,4 | 71,8 / 10,5 |
| 8119 | 26,8 / 1,1 | 75,3 / 13,7 | 75,3 / 13,7 | 74,7 / 13,7 | 74,7 / 13,7 |
| 8136 | 1,6 / 1,1 | 16,9 / 2,2 | 16,9 / 17,5 | 19,7 / 17,5 | 19,7 / 17,5 |
| 8153 | −1,1 / 20,4 | −1,1 / 20,4 | 35,5 / 20,4 | 39,8 / 20,4 | 39,2 / 16,7 |
| 8170 | −3,4 / −2,2 | 5,1 / −2,2 | 5,1 / 6,7 | 23,0 / 12,9 | 73,0 / 6,2 |
| 8187 | 62,2 / −0,5 | 62,7 / −0,5 | 75,1 / 2,7 | 77,3 / 20,5 | 82,7 / 20,5 |

### Autres mesures descriptives

- **Budget de tirage équivalent (médianes, validation).** La médiane de l'évolution à 85
  appels est atteinte par le tirage à 379 propositions. Celle de l'évolution à 170 appels
  (24,8 %) n'est jamais atteinte par le tirage en 1 359 propositions, ni a fortiori celles
  des paliers suivants.
- **Dernier appel ayant amélioré le champion.** Évolution : médiane 1 057 (de 175 à
  1 301). Tirage : médiane 596,5 (de 36 à 1 347).
- **Propositions de l'évolution par type.** 10 383 mutations et 5 925 propositions
  fraîches, dont 264 au départ : 34,7 % des appels sont des propositions fraîches de
  relance.
- **Champions issus d'une proposition fraîche de relance.** 0/12 à 85, 170, 680 et 1 359
  appels ; 1/12 à 340 appels.
- **Tokens et coût.**

| | Évolution | Tirage |
|---|---|---|
| Appels | 16 308 | 16 308 |
| Tokens en entrée | 24,93 millions | 12,06 millions |
| Tokens en sortie | 13,19 millions | 9,37 millions |
| Coût estimé | 26,01 $ | 17,07 $ |
| Coût par appel | 0,001595 $ | 0,001047 $ |

  Total : 43,08 $, pour 43 $ prévus par l'essai préalable.

### Observations du premier regard

1. **Le tirage plafonne, l'évolution non.** De 340 à 1 359 propositions, la médiane du
   tirage passe de 14,1 % à 17,1 % et une seule seed dépasse 30 %, à tous les paliers.
   Sur la même plage, la médiane de l'évolution passe de 34,0 % à 57,5 %.
2. **L'évolution progresse encore tard.** Sur 9 seeds sur 12, le champion s'améliore
   encore après l'appel 700. Le run n'a pas atteint de palier visible à 1 359 appels.
3. **H3 manque le seuil avec 12 seeds.** L'écart médian par seed passe de 18,9 à 41,4
   points, mais trois seeds vont dans l'autre sens (8085, 8119, 8187 : l'évolution y avait
   déjà presque tout son gain à 340 appels) et une est inchangée (8051).
4. **Une seed perdue de bout en bout.** Sur 8068, le tirage trouve à sa 36e proposition une
   heuristique à 47,2 % ; l'évolution finit à 11,7 %.
5. **Les relances ne produisent presque aucun champion.** Elles consomment un tiers des
   appels de l'évolution, plus que le quart mesuré sur l'essai préalable, et le champion
   n'en vient qu'une fois sur 60 (seed, palier). Les progrès viennent des mutations. Les
   relances peuvent néanmoins servir de parents : ce protocole ne le mesure pas.
6. **La note de validation suit celle de l'entraînement.** Elle recule légèrement d'un
   palier au suivant sur quelques seeds (8085 : 64,6 % puis 58,0 %), comme annoncé dans les
   limites.

### Défauts relevés hors du périmètre de ce protocole

Signalés par une relecture extérieure du code et vérifiés. Ils ne concernent que le
groupe C (croisement AST), qu'aucun run de ce protocole n'utilise.

- Le contrôle de niveau 3 des greffes utilise une instance OOD-2 comme sonde
  (`anti_dead_code.rs`). Il ne s'exécute que pour une capsule portant un manifeste de
  greffe, donc jamais en groupe D.
- Dans le groupe C, `record_proposal` est appelé avant le filtre qui peut rejeter le
  croisement (`engine.rs`).

À corriger avant tout test du croisement.

## Extension : 24 seeds

À compléter après les 12 seeds supplémentaires.
