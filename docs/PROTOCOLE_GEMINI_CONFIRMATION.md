# Protocole de confirmation sur Gemini : tirage contre évolution, moteur corrigé (fixé avant exécution)

Rédigé le 13 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Avec l'interpréteur corrigé, le moteur d'évolution de NOMAD trouve-t-il de meilleures
heuristiques qu'un tirage de propositions indépendantes :

1. à nombre d'appels égal ;
2. à dépense égale ?

## Contexte

Les protocoles précédents (Weibull v1 et v2, témoin, MiniMax v1 et v2) ont tous été
exécutés avec un interpréteur qui s'écartait de Rust : voir
[CORRECTION_INTERPRETEUR.md](CORRECTION_INTERPRETEUR.md). Leurs résultats ne peuvent pas
être corrigés après coup. Ce protocole est le premier exécuté avec l'interpréteur corrigé.
Il change aussi de modèle et double le nombre de seeds, parce que 12 seeds se sont révélées
trop peu : avec le même réglage, le tirage a réussi sur 9 seeds puis sur 5.

Le moteur d'évolution est celui du protocole MiniMax v2, sans aucune retouche.

## Dispositif commun aux trois bras

- Benchmark : profil Weibull inchangé (`NOMAD_BP_PROFILE=weibull`), mêmes quatre jeux
  (entraînement, validation, OOD-1, OOD-2), fuel `NOMAD_FUEL_LIMIT=400000` mis à l'échelle.
- Modèle : `gemini-3.1-flash-lite` par
  `https://generativelanguage.googleapis.com/v1beta/openai/chat/completions`, raisonnement
  au minimum (`reasoning_effort: minimal`, il ne peut pas être coupé), température 0,7,
  top_p 0,95. L'API refuse le champ `seed` : il est retiré de la requête.
- 24 seeds neuves : 6000, 6017, ..., 6391. Aucune n'a servi à un essai (les essais de
  connexion ont utilisé la seed 3000).
- Code figé par le tag git `gemini-v1`, exécutable `target/frozen/nomad-gemini-v1.exe`.
- Lancement par `scripts/campaign/run_gemini_arm.ps1`, les bras en parallèle, chacun
  réparti sur plusieurs processus. Les runs sont indépendants.

## Les trois bras

| Bras | Procédé | Commande |
|---|---|---|
| Tirage 85 | 85 propositions fraîches, la meilleure à l'entraînement est le champion | `run -G D --pop-size 85 -n 1` |
| Évolution | Groupe D, 22 propositions fraîches puis 9 générations de 7 appels (85 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée | `run -G D --pop-size 8 -n 10` |
| Tirage 170 | 170 propositions fraîches, la meilleure à l'entraînement est le champion | `run -G D --pop-size 170 -n 1` |

Le bras « Tirage 170 » sert à la comparaison à dépense égale. Dans le protocole MiniMax v2,
l'évolution a consommé 2,08 fois plus de tokens que le tirage pour le même nombre d'appels.
Le doublement est fixé à l'avance ; le rapport de coût réellement mesuré sera rapporté.

## Mesures et lecture, fixées à l'avance

**Grandeur mesurée.** Par run, l'effet du champion sur la validation : part de l'excès de
Best-Fit supprimée, `(BF − champion) / (BF − L1)`. Un champion invalide sur la validation
compte pour −100 %.

**Test principal (H1, à appels égaux).** Évolution contre Tirage 85, test de Wilcoxon signé
apparié, bilatéral, exact, sur les 24 seeds. H1 est établie si p < 0,05 et si l'évolution
gagne plus de seeds qu'elle n'en perd.

**Test secondaire (H2, à dépense égale).** Évolution contre Tirage 170, même test, même
seuil. H2 n'est interprétée que si H1 est établie.

**Lecture.**

- H1 et H2 établies : l'évolution fait mieux que le tirage, y compris à dépense égale.
  Résultat à reproduire sur un autre problème avant toute généralisation.
- H1 établie, H2 non : l'avantage de l'évolution à appels égaux ne résiste pas à une
  comparaison à dépense égale.
- H1 non établie : pas de différence montrée à ce budget.
- Test significatif en faveur d'un tirage : l'évolution fait moins bien que le tirage à ce
  budget.

**Mesures descriptives, sans valeur de test.** Par bras : nombre de seeds à 10 % ou plus et
à 30 % ou plus sur la validation ; effet médian ; mêmes mesures sur OOD-1 et OOD-2 ;
champions invalides ; nombre de champions de l'évolution qui sont des propositions de
départ non modifiées ; tokens et coût par bras.

## Limites connues

- Un seul modèle, un seul problème, un seul budget. Ce protocole ne dit rien des budgets
  plus grands, où l'accumulation a plus de temps pour jouer.
- Sans champ `seed`, les réponses du modèle ne sont pas reproductibles. Un modèle distant
  peut aussi être modifié par son fournisseur.
- Le budget « à dépense égale » est une approximation fixée à l'avance (170 appels).
- Les propositions fraîches reçoivent un des cinq « archétypes » du prompt de génération,
  dont un proche de l'astuce de la référence manuelle. Le prompt de mutation n'en contient
  pas.
- L'interpréteur corrigé garde des tolérances documentées (calcul mêlant entier et
  flottant accepté, entre autres) : un champion peut ne pas être du Rust compilable.
- Les trois modifications du moteur (départ large, relance, mutation informée) sont testées
  ensemble.
