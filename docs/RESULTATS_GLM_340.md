# Résultats du protocole à 344 appels sur glm-5.3-flash

Runs exécutés du 15 novembre 2026 à 17 h 59 au 16 novembre à 5 h 35, selon
[PROTOCOLE_GLM_340.md](PROTOCOLE_GLM_340.md) (commit `38d1c14`), avec l'exécutable figé par
le tag `gemini-courbe`. Les 48 runs prévus (2 bras de 24 seeds, 9000 à 9391) sont complets,
sans échec ni nouvel essai. Le bras de tirage a fini à 22 h 21, celui d'évolution à 5 h 35.
Données brutes : `results_glm_340/`, commitées avant le dépouillement (commit `7fbc356`).
Le script a d'abord été passé en mode `controle` (24 seeds × 2 bras, 0 problème).

Pour reproduire les tableaux et les tests :

```
python scripts/analysis/gemini_courbe.py --glm340
```

## Verdict

**H1 est établie. H2 ne l'est pas.** Lecture prévue par le protocole : l'évolution fait
mieux à appels égaux sur ce modèle, mais pas à dépense égale.

- **H1, à appels égaux (344 contre 344).** L'évolution gagne 18 seeds, en perd 5, une
  égalité. Wilcoxon signé apparié, bilatéral, exact : W+ = 238, n = 23, p = 0,0015.
  Différence médiane : 28,4 points.
- **H2, à dépense égale.** Rapport de coût mesuré `r` = 4,3571, donc `k*` = 78 :
  l'évolution arrêtée à 78 appels contre le tirage complet de 344 propositions.
  L'évolution gagne 10 seeds et en perd 14. W+ = 124, n = 24, p = 0,4732. Différence
  médiane : −7,8 points. Aucun écart significatif, dans un sens ni dans l'autre.

L'avantage de l'évolution à appels égaux, établi sur `gemini-3.1-flash-lite`, se retrouve
donc sur un modèle d'un autre fabricant. L'avantage à dépense égale ne se retrouve pas à ce
budget.

## Courbes : effet médian par palier

Part de l'excès de Best-Fit supprimée, médiane des 24 seeds.

| Appels | 85 | 170 | 344 |
|---|---|---|---|
| Évolution, validation | 11,6 % | 27,7 % | 58,9 % |
| Tirage, validation | 17,1 % | 30,0 % | 32,5 % |
| Évolution, entraînement | 19,8 % | 29,7 % | 62,4 % |
| Tirage, entraînement | 18,1 % | 26,3 % | 30,0 % |
| Écart médian par seed (validation) | 1,0 | 4,5 | 28,4 |
| Seeds gagnées par l'évolution | 13/24 | 15/24 | 18/24 |
| Évolution ≥ 10 % (validation) | 12/24 | 21/24 | 23/24 |
| Évolution ≥ 30 % (validation) | 10/24 | 12/24 | 18/24 |
| Tirage ≥ 10 % (validation) | 18/24 | 21/24 | 22/24 |
| Tirage ≥ 30 % (validation) | 7/24 | 12/24 | 14/24 |

Aucun champion invalide sur la validation, à aucun palier.

Pour comparaison, sur Gemini (24 seeds du protocole de courbe, palier 340) : évolution
34,0 %, tirage 15,0 %. Sur GLM, les deux bras vont plus loin, et le tirage bien plus.

## Excès de Best-Fit supprimé sur la validation, par seed (évolution / tirage)

| Seed | 85 | 170 | 344 |
|---|---|---|---|
| 9000 | 3,1 / 2,6 | 11,4 / 14,5 | 76,2 / 14,5 |
| 9017 | 0,0 / 19,8 | 21,9 / 19,8 | 56,1 / 32,6 |
| 9034 | 13,8 / 37,2 | 13,8 / 37,2 | 13,8 / 32,4 |
| 9051 | 54,0 / 32,6 | 62,0 / 32,6 | 58,8 / 32,6 |
| 9068 | 34,2 / 0,5 | 50,0 / 7,9 | 60,0 / 11,1 |
| 9085 | 2,8 / 22,2 | 45,6 / 22,2 | 58,9 / 28,3 |
| 9102 | 53,0 / 29,7 | 69,2 / 37,8 | 78,4 / 38,4 |
| 9119 | −0,5 / 3,2 | 13,8 / 9,0 | 44,4 / 9,0 |
| 9136 | 15,6 / 12,0 | 18,2 / 12,0 | 72,4 / 12,0 |
| 9153 | 45,2 / 40,3 | 73,7 / 40,3 | 76,9 / 40,3 |
| 9170 | 9,3 / 51,1 | 15,4 / 51,1 | 68,1 / 51,1 |
| 9187 | 9,0 / 17,5 | 21,7 / 17,5 | 56,1 / 30,7 |
| 9204 | 78,0 / 33,0 | 82,4 / 33,0 | 80,8 / 34,1 |
| 9221 | −1,1 / 2,7 | −2,7 / 2,7 | 21,2 / 7,1 |
| 9238 | 1,1 / 12,1 | 6,3 / 49,5 | 6,3 / 49,5 |
| 9255 | 9,1 / 15,6 | 10,2 / 15,6 | 10,2 / 15,6 |
| 9272 | 58,9 / 16,8 | 67,5 / 16,8 | 67,5 / 27,9 |
| 9289 | 66,0 / 33,5 | 67,0 / 34,0 | 66,0 / 34,0 |
| 9306 | 57,0 / 1,0 | 64,2 / 36,3 | 72,5 / 36,3 |
| 9323 | 6,8 / 32,8 | 37,5 / 45,3 | 39,6 / 40,6 |
| 9340 | 6,3 / 10,5 | 14,7 / 14,2 | 15,8 / 14,2 |
| 9357 | 72,5 / 27,5 | 72,5 / 27,5 | 73,1 / 27,5 |
| 9374 | 5,2 / 3,6 | 7,7 / 46,4 | 17,0 / 46,4 |
| 9391 | 33,5 / 16,2 | 33,5 / 48,7 | 48,7 / 48,7 |

## Autres mesures descriptives

- **Budget de tirage équivalent (médianes, validation).** La médiane de l'évolution à 85
  appels (11,6 %) est atteinte par le tirage à 41 propositions, celle à 170 appels (27,7 %)
  à 158 propositions. Celle à 344 appels (58,9 %) n'est pas atteinte en 344 propositions.
- **Dernier appel ayant amélioré le champion.** Évolution : médiane 321,5 (de 52 à 344).
  Tirage : médiane 153,5 (de 14 à 339).
- **Propositions de l'évolution par type.** 6 501 mutations et 1 755 propositions
  fraîches, dont 528 au départ : environ 15 % des appels sont des relances. Champion issu
  d'une relance : 0 sur 72 (seed, palier).
- **Propositions invalides à l'entraînement.** Évolution 428 sur 8 256 (5,2 %), tirage 983
  sur 8 256 (11,9 %). *Correction du 16 novembre, après dépouillement :* l'opérateur remplace
  sans le signaler une réponse du modèle qui ne se lit pas comme du Rust par un gabarit
  (First-Fit ou Best-Fit), noté comme une proposition valide. C'est arrivé à 202
  propositions de l'évolution (2,4 %) et 166 du tirage (2,0 %) ; ces chiffres d'invalides
  les comptent donc comme valides. Aucun champion retenu à un palier n'est un gabarit, dans
  aucun bras : les tests H1 et H2 ne sont pas touchés. Sur le protocole de courbe Gemini :
  29 et 36 gabarits sur 32 616 propositions par bras (0,1 %), aucun champion à un palier.
- **Tokens et coût**, au tarif de Relace :

| | Évolution | Tirage |
|---|---|---|
| Appels | 8 256 | 8 256 |
| Tokens en entrée | 30,15 millions | 5,43 millions |
| Tokens en sortie | 20,94 millions | 5,56 millions |
| Coût estimé | 2,84 $ | 0,65 $ |
| Coût par appel | 0,000345 $ | 0,000079 $ |

  Total : 3,49 $, pour environ 2,40 $ prévus par l'essai préalable.

## Observations

1. **L'évolution démarre plus lentement que le tirage, puis le dépasse nettement.** À 85
   appels, la médiane du tirage est devant (17,1 % contre 11,6 %) ; à 170 appels les deux
   sont proches ; à 344 appels l'évolution est à 58,9 %, le tirage à 32,5 %. Sur Gemini,
   l'évolution menait dès 85 appels. Le tirage de GLM est plus fort : ses propositions
   indépendantes valent mieux que celles de Gemini.
2. **H2 échoue sur le démarrage lent, pas sur le résultat final.** Avec `r` = 4,36,
   l'évolution à dépense égale est arrêtée à 78 appels, avant qu'elle ne décolle. À ce
   point de la courbe, elle est à égalité avec un tirage de 344 propositions.
3. **Le surcoût vient du grossissement des programmes.** Un appel d'évolution porte 5,5 fois
   plus de tokens en entrée (le code des parents et du champion) et 3,8 fois plus en
   sortie (le programme entier réécrit) qu'un appel de tirage. Le rapport `r` est de 4,36,
   contre 2,5 attendu d'après l'essai préalable et 1,5 sur Gemini.
4. **L'évolution progresse encore à la fin, le tirage non.** Le dernier progrès de
   l'évolution arrive en médiane à l'appel 321 sur 344 ; celui du tirage à l'appel 153.
   Sur 9000, 9119 et 9136, l'évolution gagne plus de 30 points entre 170 et 344 appels.
   Le protocole ne teste pas H3 ; ces chiffres laissent attendre qu'un budget plus grand
   creuse l'écart.
5. **Les seeds perdues le sont surtout sur une évolution bloquée.** Sur 9034, le champion
   de l'évolution ne bouge plus après l'appel 52 ; sur 9238 et 9255, la note de validation
   ne bouge plus après 170 appels. Sur 9374 et 9238, le tirage trouve entre 85 et 170
   appels une heuristique à plus de 45 % que l'évolution n'égale pas.
6. **Les relances ne donnent directement aucun champion.** Elles consomment environ 15 %
   des appels d'évolution, pour aucun champion à aucun palier. Mais elles servent de
   parents : 7 champions finaux sur 24 descendent d'une relance (mesure faite après le
   dépouillement, voir plus bas).
7. **L'évolution produit moitié moins de programmes invalides** que le tirage (5,2 % contre
   11,9 %) : partir d'un programme valide aide le modèle.

## Limites

Celles du protocole : un seul problème ; le modèle est servi par un hébergeur tiers en
version quantifiée (fp4) ; réponses non reproductibles faute de champ `seed` ; le réglage
du moteur a été choisi sur Gemini et n'est pas réajusté pour GLM. Le budget est de 344
appels, la comparaison avec le palier 340 de Gemini est approchée.

## Ce que ce résultat suggère pour la suite

Hors protocole, à décider :

- **Réduire le coût par appel de l'évolution**, première condition de H2 sur ce modèle :
  élaguer les programmes par ablation avant de les montrer au modèle (l'élagage hors ligne
  des 24 champions Gemini retire 45 règles sur 273 sans perte, branche `ast-croisement`),
  ou demander au modèle une modification plutôt qu'un programme entier.
- ~~Réduire les relances~~. Mesure faite le 16 novembre en remontant l'ascendance des
  champions finaux (parents successifs dans le journal) : 7 champions GLM sur 24, et 11
  champions Gemini sur 24 (protocole de courbe), descendent d'une proposition fraîche de
  relance, qui est la racine de leur lignée. Les relances produisent rarement le champion
  elles-mêmes, mais elles fondent souvent la lignée qui y mène : les réduire n'est pas
  une piste.
- **Tester H3 sur ce modèle**, l'évolution progressant encore à 344 appels.

## Lecture d'ensemble

Sur quatre protocoles, deux modèles et 96 seeds, avec des critères fixés à l'avance,
l'évolution de NOMAD fait mieux qu'un tirage de propositions indépendantes du même modèle à
appels égaux : sur Gemini à 85, 340 et 1 359 appels, sur GLM à 344 appels (p = 0,0015). À
dépense égale, elle fait mieux sur Gemini à 340 et 1 359 appels, mais pas sur GLM à 344
appels, où un appel d'évolution coûte 4,4 fois un appel de tirage.
