# Synthèse : l'évolution de NOMAD contre le tirage de propositions indépendantes

Phase close le 20 novembre 2026, branche `weibull-benchmark`. Ce document résume ce qui a
été démontré, ce qui ne l'a pas été et ce qui reste ouvert. Le détail de chaque résultat
est dans le fichier `RESULTATS_*.md` cité.

## Question

À budget égal, une évolution de programmes guidée par un LLM (NOMAD) fait-elle mieux
qu'un tirage de propositions indépendantes du même modèle ? Et l'écart grandit-il avec le
budget ?

- **H1** : à appels égaux au modèle, l'évolution bat le tirage.
- **H2** : à dépense égale, en dollars, aussi.
- **H3** : l'avance de l'évolution est plus grande à 1 359 appels qu'à 340 (ou 344).

## Méthode

- Problème : bin packing en ligne, profil Weibull. Le programme évolué est une fonction de
  score `score_bin`. Effet mesuré : part de l'excès de Best-Fit supprimée, sur un jeu de
  validation distinct de l'entraînement.
- Chaque protocole est écrit et commité avant tout run (`PROTOCOLE_*.md`) : bras, seeds,
  mesures, tests et seuils.
- Les données brutes sont commitées avant d'être lues, puis le script de dépouillement est
  passé en mode contrôle, sans afficher de score.
- Test : Wilcoxon signé apparié, bilatéral, exact, sur les seeds (24 en général).
- À dépense égale : l'évolution est arrêtée à `k* = ⌊budget / r⌋` appels, où `r` est le
  rapport mesuré du coût par appel évolution / tirage. Elle affronte alors le tirage
  complet.

## Résultats

| Protocole | Modèle | Budget | H1 | H2 | H3 |
|---|---|---|---|---|---|
| [Confirmation](RESULTATS_GEMINI_CONFIRMATION.md) | gemini-3.1-flash-lite | 85 appels | **établie** (p = 0,0075) | non (p = 0,21) | — |
| [Échelle](RESULTATS_GEMINI_ECHELLE.md) | gemini-3.1-flash-lite | 340 | **établie** (p = 0,0002) | **établie** (p = 0,0016) | non (p = 0,40) |
| [Courbe](RESULTATS_GEMINI_COURBE.md) | gemini-3.1-flash-lite | 1 359 | **établie** (22/24, p < 0,0001) | **établie** (p = 0,0012) | **établie** (p = 0,0042) |
| [GLM 340](RESULTATS_GLM_340.md) | glm-5.3-flash | 344 | **établie** (p = 0,0015) | non (r = 4,36, p = 0,47) | — |
| [GLM H2, élagage](RESULTATS_GLM_H2.md) | glm-5.3-flash | 344 | **établie** (20/24, p < 0,0001) | **établie** (r = 2,12, p = 0,014) | — |
| [GLM H3](RESULTATS_GLM_H3.md) | glm-5.3-flash | 1 359 | **établie** (24/24, p < 0,0001) | **établie** (p = 0,0065) | non (p = 0,60) |

Courbes médianes en validation à 1 359 appels :

| | Évolution | Tirage | Écart médian par seed |
|---|---|---|---|
| Gemini | 56,0 % | 17,7 % | +31,8 points |
| GLM | 65,4 % | 42,5 % | +25,9 points |

## Ce qui est établi

1. **À appels égaux, l'évolution bat le tirage, sur deux modèles de deux fabricants**, à
   tous les budgets testés (85 à 1 359 appels). À 1 359 appels, elle gagne 22 seeds sur
   24 sur Gemini et 24 sur 24 sur GLM.
2. **À dépense égale aussi**, dès que le coût par appel de l'évolution reste contenu. Sur
   GLM, H2 échouait quand un appel d'évolution coûtait 4,4 fois un appel de tirage, à
   cause du code qui grossit. Elle est établie une fois ce rapport ramené à 2,1 par
   l'élagage du champion.
3. **Sur Gemini, l'avance grandit avec le budget** : le tirage plafonne vers 17 %,
   l'évolution continue.

## Ce qui ne l'est pas

1. **H3 sur GLM.** L'avance médiane reste entre 20 et 27 points de 85 à 1 359 appels,
   parce que le tirage de GLM continue de progresser. Ce qui progresse avec le budget,
   c'est la régularité : l'évolution gagne 17 seeds sur 24 à 344 appels, et toutes à
   1 359. Lecture : l'avance ne grandit que là où le tirage s'épuise.
2. **Le croisement.** Testé une seule fois ([RESULTATS_GLM_LOCAL.md](RESULTATS_GLM_LOCAL.md),
   12 seeds, 344 appels) : effet non démontré (p = 0,77). Ce test mélangeait croisement et
   élagage, et utilisait la greffe d'avant les corrections du 17 novembre. Les croisements
   y donnent pourtant des champions : 45 nouveaux champions sur 1 656 croisements, gratuits
   en appels, et le champion final dans 3 seeds sur 12. **Aucun des résultats établis ne
   repose sur le croisement.** L'évolution qui gagne est une population avec sélection par
   rang, mutation informée par le LLM et relances.

## Limites

- Un seul problème (bin packing en ligne, profil Weibull) et un seul jeu d'instances.
- Deux modèles, tous deux petits et bon marché ; réponses non reproductibles à l'identique.
- Le réglage du moteur a été choisi sur Gemini.
- Les relances de l'évolution sont des propositions fraîches : le bras d'évolution
  contient une part de tirage.
- `k*` repose sur le coût moyen par appel. C'est défavorable à l'évolution, dont le coût
  par appel croît au fil du run.
- L'élagage fait des évaluations supplémentaires, gratuites en appels mais pas en calcul.
- Pas de comparaison avec l'état de l'art publié (FunSearch, EoH, ReEvo) : instances et
  mesure différentes.

## Coût

Environ 148 $ d'appels pour les protocoles ci-dessus, essais préalables compris quand
ils sont comptés dans les fiches. Dont 86 $ pour la courbe Gemini, et 12 $ pour H3 sur GLM.

## Questions ouvertes, laissées au projet de grille

- **Le croisement est-il utile ?** C'est la question centrale pour une grille qui échange
  des gènes entre îles. Il faudrait un protocole qui l'isole : greffe corrigée, croisement
  seul sans élagage, 24 seeds, groupe C contre groupe D.
- **Taille du code.** Sur GLM, le champion final fait 9 120 caractères en médiane à
  1 359 appels. Dans H3, ce grossissement vient entièrement des mutations du LLM :
  +16 100 caractères par run en médiane, dont l'élagage retire environ 8 600. Le prompt
  de mutation demande d'ajouter des termes après un succès. Pistes : préférer le plus
  court à score égal, prompt qui remplace au lieu d'ajouter. L'élagage profond retire un
  tiers du code sans perte, mais il est trop lent pour le moteur
  (`MESURE_ELAGAGE_FORT.md` sur `ast-croisement`).
- **Îles contre une seule population**, autres problèmes (l'ordonnancement est à refaire),
  autres modèles.
