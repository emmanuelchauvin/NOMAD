# Protocole MiniMax v2 : tirage contre évolution à départ large, relance et mutation informée (fixé avant exécution)

Rédigé le 13 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

À budget égal d'appels au modèle, le moteur d'évolution modifié découvre-t-il une
heuristique nettement meilleure que Best-Fit plus souvent qu'un tirage de propositions
indépendantes ?

## Contexte

Le protocole précédent ([résultats](RESULTATS_MINIMAX_TIRAGE_EVOLUTION.md)) a donné 9
succès sur 12 à chaque bras au seuil de 10 %. L'examen de ses runs, génération par
génération, a montré deux défauts du moteur :

- des 8 propositions de départ, l'archive n'en garde que 1 à 3 (une par niche). Sur les
  trois seeds où l'évolution échoue (4051, 4119, 4136), 77 mutations partent de ce départ
  médiocre sans rien trouver, et rien ne relance la recherche ;
- le prompt de mutation ne donne au modèle ni le score mesuré du parent, ni le meilleur
  programme connu.

Trois modifications ont été faites, chacune activée par une variable d'environnement et
inactive par défaut :

| Modification | Variable | Valeur du protocole |
|---|---|---|
| Départ large : nombre de propositions fraîches indépendant de la population | `NOMAD_INIT_POP` | 22 |
| Relance : après N générations sans nouveau meilleur score, une proposition sur deux est fraîche | `NOMAD_STAGNATION_PATIENCE` | 3 |
| Mutation informée : bacs du parent, minimum théorique, code et bacs du meilleur programme connu | `NOMAD_INFORMED_MUTATION` | 1 |

## Dispositif commun aux deux bras

- Benchmark, modèle et réglages identiques au protocole précédent : profil Weibull, fuel
  `NOMAD_FUEL_LIMIT=400000` mis à l'échelle, `MiniMax-M3` sans raisonnement, température
  0,7, top_p 0,95.
- 12 seeds neuves : 5000, 5017, ..., 5187. Aucune n'a servi à un essai (l'essai à blanc
  du moteur modifié a utilisé la seed 3000).
- Budget : 85 appels au modèle par run.
- Code figé par le tag git `minimax-v2`, exécutable `target/frozen/nomad-minimax-v2.exe`.
- Lancement par `scripts/campaign/run_minimax_v2_arm.ps1`. Les deux bras tournent en même
  temps, chacun réparti sur deux processus (rangs de seeds 0 à 5 et 6 à 11). Les runs sont
  indépendants : cette répartition ne change que la durée.

## Les deux bras

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 85 propositions fraîches, la meilleure à l'entraînement est le champion. Aucune des trois modifications. | `run -G D --pop-size 85 -n 1` |
| Évolution | Groupe D, 22 propositions fraîches puis 9 générations de 7 appels (22 + 63 = 85), sélection par rang, les trois modifications actives | `run -G D --pop-size 8 -n 10` |

Les deux bras sont exécutés, sans condition préalable.

## Mesures et lecture, fixées à l'avance

**Mesure principale.** Par bras, nombre de seeds où le champion supprime au moins 30 % de
l'excès de Best-Fit sur la validation, `(BF − champion) / (BF − L1)`.

Le seuil passe de 10 % à 30 % parce que le seuil de 10 % est atteint 3 fois sur 4 par le
tirage et ne peut plus départager les bras. Ce choix a été fait après lecture des résultats
du protocole précédent, où l'évolution atteignait 30 % sur 5 seeds et le tirage sur 2.
C'est pourquoi il est testé ici sur des seeds neuves.

**Mesures secondaires.** Nombre de seeds à 10 % ou plus ; effet médian sur la validation ;
effets sur OOD-1 et OOD-2 ; champions invalides ; nombre de seeds où l'évolution fait mieux
que le tirage sur la validation ; test de Wilcoxon signé apparié, bilatéral, exact, entre
les deux bras sur la validation ; tokens consommés par bras.

**Lecture de la mesure principale.**

- Évolution à au moins 3 succès de plus que le tirage : indice en faveur du moteur
  modifié, à confirmer sur plus de seeds.
- Écart de 2 succès ou moins, dans un sens ou dans l'autre : pas de différence montrée à
  ce budget.
- Tirage à au moins 3 succès de plus que l'évolution : le moteur modifié reste moins bon
  que le tirage à ce budget.

## Limites connues

- 12 seeds : seule une différence très nette serait statistiquement établie.
- Trois modifications sont testées ensemble. Ce protocole ne dit pas laquelle compte.
- Le budget est égal en appels, pas en tokens : le prompt de mutation est plus long, et la
  mutation informée l'allonge encore. Les tokens sont mesurés et rapportés.
- Les propositions fraîches reçoivent un des cinq « archétypes » du prompt de génération,
  dont un proche de l'astuce de la référence manuelle. Le tirage en reçoit 17 par run,
  l'évolution 4 ou 5 au départ, plus ceux des relances.
- Un seul budget (85 appels), un seul modèle, un seul benchmark.
- Un modèle distant peut être modifié par son fournisseur ; la reproduction exacte n'est
  pas garantie.
