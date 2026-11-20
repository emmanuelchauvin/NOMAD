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

## Extension : 24 seeds (résultat final)

Les 12 seeds supplémentaires (8204 à 8391) ont été exécutées le 15 novembre 2026, de
2 h 10 à 11 h 25, à l'identique : même exécutable, mêmes réglages, sans échec ni nouvel
essai. Données brutes commitées avant le dépouillement (commit `f4ce9fc`).

```
python scripts/analysis/gemini_courbe.py --seeds24
```

### Verdict final

**H3 est établie, ainsi que H1 et H2, au seuil de 0,025 fixé pour le second regard.**

- **H3, test principal : écart à 1 359 appels contre écart à 340 appels, 24 seeds.**
  L'écart grandit sur 16 seeds, se réduit sur 6, est inchangé sur 2. Wilcoxon signé
  apparié, bilatéral, exact : W+ = 212, n = 22, p = 0,0042. Différence médiane :
  6,5 points.
- **H1, à appels égaux (1 359 contre 1 359).** L'évolution gagne 22 seeds et en perd 2.
  W+ = 283, n = 24, p < 0,0001. Différence médiane : 31,8 points.
- **H2, à dépense égale.** `r` = 1,5134, `k*` = 898 : l'évolution arrêtée à 898 appels
  contre le tirage complet. L'évolution gagne 18 seeds et en perd 6. W+ = 258, n = 24,
  p = 0,0012. Différence médiane : 29,9 points.

Lecture prévue par le protocole : l'avance de l'évolution grandit avec le budget, sur ce
problème et ce modèle.

### Courbes sur 24 seeds

| Appels | 85 | 170 | 340 | 680 | 1 359 |
|---|---|---|---|---|---|
| Évolution, validation (médiane) | 14,5 % | 22,4 % | 34,0 % | 41,2 % | 56,0 % |
| Tirage, validation (médiane) | 4,2 % | 11,0 % | 15,0 % | 17,5 % | 17,7 % |
| Évolution, entraînement (médiane) | 15,9 % | 26,1 % | 38,0 % | 45,3 % | 59,8 % |
| Tirage, entraînement (médiane) | 3,2 % | 10,8 % | 15,5 % | 19,1 % | 22,2 % |
| Écart médian par seed (validation) | 11,4 | 14,4 | 14,3 | 14,9 | 31,8 |
| Seeds gagnées par l'évolution | 18/24 | 19/24 | 16/24 | 19/24 | 22/24 |
| Évolution ≥ 10 % (validation) | 16/24 | 20/24 | 21/24 | 22/24 | 24/24 |
| Évolution ≥ 30 % (validation) | 8/24 | 11/24 | 14/24 | 14/24 | 21/24 |
| Tirage ≥ 10 % (validation) | 7/24 | 12/24 | 19/24 | 21/24 | 22/24 |
| Tirage ≥ 30 % (validation) | 3/24 | 3/24 | 4/24 | 5/24 | 5/24 |

Aucun champion invalide sur la validation.

### Validation par seed, seeds de l'extension (évolution / tirage)

| Seed | 85 | 170 | 340 | 680 | 1 359 |
|---|---|---|---|---|---|
| 8204 | 53,2 / 32,3 | 71,0 / 32,3 | 72,0 / 32,3 | 75,3 / 32,3 | 75,8 / 32,3 |
| 8221 | 13,6 / 12,6 | 14,1 / 13,1 | 26,7 / 13,1 | 21,5 / 17,3 | 54,5 / 17,8 |
| 8238 | 15,7 / 7,6 | 15,7 / 6,1 | 15,7 / 18,7 | 15,7 / 36,9 | 33,3 / 36,9 |
| 8255 | 66,7 / 17,5 | 66,7 / 17,5 | 71,4 / 17,5 | 80,4 / 17,5 | 79,9 / 17,5 |
| 8272 | 43,5 / −0,5 | 45,1 / 0,0 | 46,1 / 54,4 | 46,1 / 54,4 | 57,0 / 54,4 |
| 8289 | 47,7 / 7,3 | 47,7 / 7,3 | 78,8 / 10,4 | 79,8 / 10,4 | 80,8 / 8,8 |
| 8306 | 9,8 / 32,6 | 25,9 / 32,6 | 31,6 / 32,6 | 37,8 / 32,6 | 51,8 / 32,6 |
| 8323 | 4,2 / 4,7 | 9,4 / 19,8 | 9,4 / 19,8 | 9,4 / 19,8 | 45,8 / 19,8 |
| 8340 | 43,3 / 16,1 | 43,3 / 14,4 | 43,3 / 14,4 | 43,3 / 14,4 | 43,3 / 14,4 |
| 8357 | 13,2 / −0,5 | 14,8 / 22,5 | 13,7 / 22,5 | 13,7 / 22,5 | 54,9 / 22,5 |
| 8374 | 12,4 / 0,0 | 15,6 / 1,6 | 17,2 / 12,9 | 17,2 / 12,9 | 17,2 / 14,0 |
| 8391 | −1,6 / 0,5 | 18,8 / 7,5 | 57,0 / 7,5 | 57,0 / 18,8 | 57,0 / 25,8 |

### Autres mesures sur 24 seeds

- **Budget de tirage équivalent.** La médiane de l'évolution à 85 appels est atteinte par
  le tirage à 304 propositions ; celle de l'évolution à 170 appels (22,4 %) n'est jamais
  atteinte en 1 359 propositions.
- **Dernier appel ayant amélioré le champion.** Évolution : médiane 1 173,5. Tirage :
  médiane 467,5.
- **Propositions de l'évolution.** 20 739 mutations et 11 877 propositions fraîches, dont
  528 au départ : 34,8 % des appels sont des relances. Champion issu d'une relance : 2 cas
  sur 120 (seed, palier), aucun à 1 359 appels.
- **Coût estimé.** Évolution 51,68 $ (0,001585 $ par appel), tirage 34,15 $ (0,001047 $
  par appel), total 85,83 $ pour les 48 runs.

### Observations finales

1. **L'effet d'échelle vient de deux mouvements.** Le tirage s'arrête de progresser
   (médiane 17,5 % à 680 propositions, 17,7 % à 1 359) pendant que l'évolution continue
   (41,2 % puis 56,0 %).
2. **L'écart ne grandit pas régulièrement.** L'écart médian par seed est stable de 170 à
   680 appels (14 à 15 points) et double au dernier palier (31,8 points). Le test fixé à
   l'avance compare 340 et 1 359 appels ; il ne dit pas à partir de quel budget l'écart
   se creuse.
3. **L'évolution avance par sauts tardifs.** Plusieurs seeds restent longtemps sur un
   champion médiocre puis progressent d'un coup entre 680 et 1 359 appels (8323 : 9,4 %
   puis 45,8 % ; 8357 : 13,7 % puis 54,9 % ; 8170 : 23,0 % puis 73,0 %). À 1 359 appels,
   13 seeds sur 24 ont encore amélioré leur champion après l'appel 1 100 : le run n'a pas
   atteint de palier.
4. **Les seeds perdues le sont sur un coup de chance du tirage.** 8068 et 8238 : le tirage
   y trouve une heuristique à 47,2 % et 36,9 % que l'évolution n'égale pas. Sur 8272,
   l'évolution ne repasse devant qu'au dernier palier.
5. **Deux seeds ne bougent pas.** 8340 garde le même champion de l'appel 80 à la fin,
   8374 depuis l'appel 335 : la relance ne les débloque pas.
6. **Les relances coûtent un tiers du budget et ne donnent presque aucun champion.**

### Limites

Celles du protocole : un seul modèle, un seul problème ; réponses non reproductibles
faute de champ `seed` ; les trois modifications du moteur sont testées ensemble ; le bras
d'évolution contient un tiers de propositions fraîches. Le résultat de H3 a demandé le
second regard prévu, au seuil corrigé de 0,025.

### Lecture d'ensemble

Sur trois protocoles et 72 seeds, avec des critères fixés à l'avance, l'évolution de NOMAD
fait mieux qu'un tirage de propositions indépendantes du même modèle : à 85 appels
(p = 0,0075), à 340 appels (p = 0,0002) et à 1 359 appels (p < 0,0001), y compris à
dépense égale à 340 et 1 359 appels. Et son avance est plus grande à 1 359 appels qu'à
340 sur les mêmes seeds (p = 0,0042).

Restent à établir : le même résultat sur un autre modèle et sur un autre problème.
