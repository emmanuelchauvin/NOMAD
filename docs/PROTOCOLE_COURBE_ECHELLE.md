# Courbe de progression de l'évolution à l'intérieur des runs (critères fixés avant calcul)

Rédigé le 14 novembre 2026, avant tout calcul. Analyse sans nouveau run ni dépense : elle
relit les données de [RESULTATS_GEMINI_ECHELLE.md](RESULTATS_GEMINI_ECHELLE.md) et de
[RESULTATS_GEMINI_CONFIRMATION.md](RESULTATS_GEMINI_CONFIRMATION.md).

## Question

L'évolution progresse-t-elle encore à la fin de ses 340 appels, ou a-t-elle atteint un
palier ? La réponse sert à décider si un troisième budget, plus grand, vaut d'être payé.

## Ce que les données permettent, et ce qu'elles ne permettent pas

L'analyse envisagée au départ était une comparaison appariée de l'évolution et du tirage à
85, 170, 255 et 340 appels sur les mêmes seeds. **Elle n'est pas faisable** : les rapports
ne conservent ni le code ni le score de chaque proposition. Ils gardent seulement le
champion, les occupants finaux des niches de l'archive et, pour chaque génération, le
score d'entraînement du meilleur individu. Pour un tirage (une seule génération), il n'y a
donc aucun point intermédiaire.

Restent exploitables :

- pour l'évolution à 340 appels, le meilleur score d'entraînement à chaque génération,
  soit 37 points par seed, de 88 à 340 appels ;
- pour le tirage, les bras indépendants à 85, 170, 340 et 455 propositions, plus la
  première génération des deux bras d'évolution, qui est un tirage de 22 propositions
  (seeds 6000 à 6391) et de 88 propositions (seeds 7000 à 7391).

Toutes ces mesures portent sur l'**entraînement**, pas sur la validation. Avant la
rédaction, seuls les points finaux ont été vus (médianes d'entraînement du tirage : 4,3 %,
10,3 %, 16,9 %, 16,9 % ; de l'évolution : 15,3 % à 85 appels, 35,9 % à 340). Aucun point
intermédiaire n'a été regardé.

## Mesures, fixées à l'avance

**Grandeur.** Effet à l'entraînement du meilleur individu connu après `c` appels : part de
l'excès de Best-Fit supprimée, en prenant le meilleur score valide rencontré jusque-là.

**M1, progression par quart.** Les 252 appels d'évolution sont coupés en trois tranches de
84 appels : de 88 à 172, de 172 à 256, de 256 à 340. Pour chaque tranche : nombre de seeds
dont l'effet augmente, et gain médian en points.

**Lecture de M1, sur la dernière tranche (256 à 340 appels).**

- 12 seeds ou plus sur 24 progressent : l'évolution monte encore à 340 appels. Un
  troisième budget est justifié.
- 6 seeds ou moins progressent : l'évolution s'essouffle. Un budget plus grand avec ce
  moteur a peu de chances de changer le tableau.
- Entre les deux : indéterminé.

**M2, tableau descriptif.** Effet médian à l'entraînement en fonction du nombre d'appels,
pour le tirage (22, 85, 88, 170, 340, 455) et pour l'évolution (88, 172, 256, 340). Sans
test : les points du tirage viennent de runs et de seeds différents.

## Limites

- Mesures à l'entraînement : elles ne disent rien de la généralisation.
- Un run de 340 appels arrêté à 172 n'est pas un run conçu pour 172 appels.
- Les seuils de lecture (12 et 6 seeds) sont un choix.
- Cette analyse ne teste pas l'effet d'échelle (H3) : elle dit seulement si la courbe de
  l'évolution est encore montante.
