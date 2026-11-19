# Mesure hors ligne : élagage du moteur contre élagage profond (19 novembre 2026)

Banc sans appel au modèle, sans valeur de test. But : savoir si un élagage plus fort
réduirait la taille du code de l'évolution, qui triple le coût d'un appel au fil d'un run
GLM ([RESULTATS_GLM_H3.md](../../nomad/docs/RESULTATS_GLM_H3.md), dépôt `nomad`, branche
`weibull-benchmark`).

## Dispositif

- **Programmes.** Les 24 champions finaux du bras évolution du protocole H3 GLM
  (1 359 appels, seeds 10400 + 17 r), profil Weibull, fuel 400 000.
- **Banc.** `crates/nomad-cli/examples/prune_probe.rs` (commit `80fbce5`). Trois élagages,
  même critère que `prune_best_elite` : un retrait est gardé si le programme reste valide
  sans perdre de bac sur l'entraînement.
  - `une_passe` : instructions du premier niveau, du bas vers le haut, une passe (le moteur).
  - `point_fixe` : la même chose répétée tant qu'un retrait passe.
  - `profond` : depuis `point_fixe`, toute instruction à toute profondeur (corps des `if`,
    boucles, blocs), répété jusqu'au point fixe.
- **Données brutes.** `mesures/prune_probe/part1..8.jsonl` et journaux, commitées avant
  lecture (`b789f67`). 8 processus de 3 seeds, du 18/11 19:38 au 19/11 07:28.

## Résultats

| | Une passe (moteur) | Point fixe | Profond |
|---|---|---|---|
| Champions réduits | 2 / 24 | 2 / 24 | 24 / 24 |
| Réduction de taille, médiane | 0 % | 0 % | 33,5 % |
| Réduction moyenne (min ; max) | 1,2 % (0 ; 16,7) | 1,2 % | 34,7 % (4,3 ; 83,0) |
| Caractères, total des 24 | 227 450 | 227 450 | 136 984 (original 228 396) |
| Instructions retirées, médiane | 0 | 0 | 44 |
| Validation : mieux / égal / pire | 0 / 23 / 1 | 0 / 23 / 1 | 10 / 3 / 11 |
| Validation : bacs gagnés, total | −2 | −2 | +10 |
| Évaluations de l'élagage profond, médiane | | | 341 |
| Temps de l'élagage profond, médiane (total) | | | 63 min (40,7 h CPU) |

Aucun programme élagué n'est invalide sur la validation ; aucun ne perd de bac sur
l'entraînement (le critère l'interdit). Les écarts de validation sont de quelques bacs sur
environ 4 080 : l'élagage profond est neutre pour la qualité.

## Lecture

1. **L'élagage du moteur ne fait presque rien sur ces champions** : rien retiré sur 22
   sur 24. Le code mort n'est pas au premier niveau du corps mais dans les blocs imbriqués
   que les greffes et mutations empilent. Le répéter jusqu'au point fixe n'ajoute rien.
2. **Un tiers du code est inutile** en médiane, jusqu'à 83 % (seed 10400 : 15 794 → 2 680
   caractères). Si la taille du prompt suit celle du champion, l'élagage profond ferait
   baisser d'environ un tiers les tokens d'entrée des appels d'évolution en fin de run.
3. **Mais il est beaucoup trop lent pour être fait à chaque nouveau champion** : une
   évaluation sur l'entraînement par instruction essayée, 341 en médiane, 63 min de CPU par
   champion (jusqu'à 5,8 h). Le moteur fait des dizaines de nouveaux champions par run.

## Pistes, à mesurer avant tout protocole

- **Élagage profond plus rapide** : essayer d'abord de retirer des blocs entiers ou des
  moitiés de liste d'instructions (réduction par dichotomie), ne descendre instruction par
  instruction que là où un gros retrait échoue ; évaluer d'abord sur un sous-ensemble
  d'instances et ne confirmer sur tout l'entraînement que les retraits qui y passent.
- **Élaguer moins souvent** : seulement quand le champion dépasse une taille seuil, ou
  avant de le donner comme parent, au lieu de chaque nouveau champion.
- **Greffe qui remplace au lieu d'empiler** : agit à la source, complémentaire.

Ce banc ne mesure pas l'effet sur le coût d'un run ni sur la qualité finale en ligne : un
code plus court change aussi ce que le modèle voit et propose.

## Suite : élagage profond rapide (19 novembre 2026)

Mode `PRUNE_PROBE_MODE=rapide` de `prune_probe` (commit `53b0141`), depuis le champion
original : bloc par bloc en ordre préfixe (une instruction englobante est essayée avant son
contenu), par tranches décroissantes (moitié, quart, ..., une instruction), et tri préalable
sur une instance sur quatre de l'entraînement (rejet si invalide ou un bac perdu sur ce
quart ; sinon note complète avec le critère du moteur). Même découpage en 8 processus,
du 19/11 07:43 à 14:59. Données brutes `mesures/prune_rapide/`, commitées avant lecture
(`11697d9`).

| | Profond (une passe + point fixe + profond) | Rapide |
|---|---|---|
| Réduction de taille, médiane (moyenne) | 33,5 % (34,7 %) | 32,8 % (32,8 %) |
| Caractères, total des 24 (original 228 396) | 136 984 | 142 440 |
| Validation : mieux / égal / pire que l'original | 10 / 3 / 11 | 10 / 2 / 12 |
| Validation : bacs gagnés, total | +10 | +12 |
| Temps par champion, médiane (max) | 69 min (6,2 h) | 41 min (3,2 h) |
| Temps total | 43,6 h CPU | 23,1 h CPU |
| Rapport de temps rapide / profond, médiane (min ; max) | | 0,53 (0,32 ; 0,85) |
| Notes complètes, médiane ; tris sur un quart, médiane | | 58 ; 437 |

Aucun programme invalide sur la validation. Contre le profond, le rapide donne un code plus
court sur 6 seeds, égal sur 6, plus long sur 12, avec des écarts de part et d'autre (seed
10553 : 9 615 contre 6 484 ; seed 10723 : 4 658 contre 6 430).

**Lecture.** La même réduction (un tiers) pour la moitié du temps, sans effet sur la
validation. C'est encore trop lent pour chaque nouveau champion : 41 min de CPU en médiane
(machine chargée par 8 processus). Le coût est maintenant dans les tris (437 en médiane,
soit environ 110 notes complètes) plus que dans les notes complètes (58). Pistes : tri sur
une instance sur huit, ou élaguer seulement quand le champion dépasse une taille seuil.
