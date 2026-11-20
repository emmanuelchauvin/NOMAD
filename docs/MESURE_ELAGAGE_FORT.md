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

## Suite : tri sur une instance sur huit, champions de plus de 5 000 caractères (19-20 novembre 2026)

Même élagage rapide, tri sur une instance sur huit (`PRUNE_SCREEN_STRIDE=8`, commit
`5a3bac5`), sur les 17 champions de plus de 5 000 caractères (les 7 autres ne seraient pas
élagués avec ce seuil). 8 processus équilibrés selon les temps du banc précédent, du 19/11
19:27 au 20/11 00:29. Données brutes `mesures/prune_seuil/`, commitées avant lecture
(`b73f41d`).

| Sur les 17 mêmes champions | Profond | Rapide, tri 1/4 | Rapide, tri 1/8 |
|---|---|---|---|
| Réduction de taille, médiane | | 35,4 % | 31,8 % |
| Caractères, total (original 202 645) | | 121 773 | 118 580 |
| Validation : mieux / égal / pire | | 7 / 1 / 9 | 6 / 0 / 11 |
| Validation : bacs gagnés, total | | −3 | +2 |
| Travail, médiane (total), en notes complètes | 400 (10 344) | 204 (4 324) | 167 (3 286) |
| Temps par champion, médiane | | 73 min | 82 min |

Travail : notes complètes de l'entraînement plus tris comptés à leur part d'instances. Aucun
programme invalide, aucun bac perdu sur l'entraînement.

**Le temps mesuré trompe ici.** Le tri sur un huitième fait 24 % de travail en moins en
médiane, mais chaque note complète a pris 25,5 s contre 16,3 s : les 17 plus gros champions
tournaient ensemble, la machine était plus chargée que dans le banc précédent. Le travail
est la mesure à retenir. (Pendant le banc, une lecture des seuls temps avait conclu à
l'absence de gain : c'était une erreur.)

**Fréquence avec le seuil**, comptée sur les 24 runs H3 (taille du code seulement) : 33,5
nouveaux champions par run en médiane (17 à 54), dont 16,5 de plus de 5 000 caractères
(0 à 33) ; 20 runs sur 24 franchissent le seuil, en médiane vers l'appel 200 sur 1 359.
Ces tailles sont celles de champions élagués par le moteur seulement ; avec un élagage
profond, un champion repasserait sous le seuil et les dépassements seraient moins
nombreux, ce que seul un run en ligne mesure.

**Lecture.** Les deux pistes combinées divisent le travail par 2,4 contre l'élagage
profond, pour la même réduction et sans effet sur la validation. Un élagage au-dessus du
seuil vaut encore environ 170 notes complètes, soit 30 à 45 min de CPU seul sur la machine ;
une quinzaine par run représenterait l'équivalent de 2 500 notes, de l'ordre du coût de
l'évaluation des 1 359 propositions elles-mêmes. Trop cher tel quel pour le moteur.
