# Protocole d'échelle sur Gemini : tirage contre évolution à 340 appels (fixé avant exécution)

Rédigé le 14 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Le protocole de confirmation
([RESULTATS_GEMINI_CONFIRMATION.md](RESULTATS_GEMINI_CONFIRMATION.md)) a établi qu'à
85 appels l'évolution fait mieux qu'un tirage du même nombre de propositions. Avec un budget
quatre fois plus grand :

1. l'évolution fait-elle encore mieux que le tirage à nombre d'appels égal ?
2. fait-elle mieux à dépense égale ?
3. l'écart avec le tirage a-t-il grandi par rapport à 85 appels ?

## Dispositif commun aux trois bras

- Benchmark, modèle et réglages du modèle : ceux du protocole de confirmation, inchangés
  (profil Weibull, fuel 400 000, `gemini-3.1-flash-lite`, raisonnement au minimum,
  température 0,7, top_p 0,95, sans champ `seed`).
- 24 seeds neuves : 7000, 7017, ..., 7391. Aucune n'a servi à un essai.
- Code figé par le tag git `gemini-echelle`, exécutable
  `target/frozen/nomad-gemini-echelle.exe`. Seule différence avec le tag `gemini-v1` : le
  client HTTP fait confiance au magasin de certificats du système (option `native-certs`
  de `ureq`), parce que l'antivirus du poste intercepte les connexions TLS. Le moteur,
  l'interpréteur et les prompts sont inchangés.
- Lancement par `scripts/campaign/run_gemini_scale_arm.ps1`, les bras en parallèle, chacun
  réparti sur plusieurs processus. Les runs sont indépendants.

## Les trois bras

| Bras | Procédé | Commande |
|---|---|---|
| Tirage 340 | 340 propositions fraîches, la meilleure à l'entraînement est le champion | `run -G D --pop-size 340 -n 1` |
| Évolution 340 | Groupe D, 88 propositions fraîches puis 36 générations de 7 appels (340 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée | `run -G D --pop-size 8 -n 37`, `NOMAD_INIT_POP=88` |
| Tirage 455 | 455 propositions fraîches, la meilleure à l'entraînement est le champion | `run -G D --pop-size 455 -n 1` |

Le bras d'évolution est celui du protocole de confirmation, ses deux parts multipliées par
quatre (22 → 88 propositions de départ, 9 → 36 générations). Aucun autre réglage ne change.

**D'où vient 455.** Un essai préalable de l'évolution à 340 appels a été fait sur trois
seeds hors protocole (3000, 3017, 3034 ; données dans `results_gemini_echelle_pilote/`).
Il a coûté 0,4625 $, 0,4669 $ et 0,4978 $ par run, soit 0,001399 $ par appel. Une
proposition de tirage a coûté 0,001045 $ en moyenne sur les 6 120 appels des deux bras de
tirage du protocole de confirmation. Le rapport est de 1,339 : 340 × 1,339 = 455. Le
rapport de coût réellement mesuré sera rapporté.

Les résultats de l'essai préalable (qualité des champions) n'ont pas été regardés avant la
rédaction de ce protocole : seuls les appels, les tokens et la durée l'ont été.

## Mesures et lecture, fixées à l'avance

**Grandeur mesurée.** Par run, l'effet du champion sur la validation : part de l'excès de
Best-Fit supprimée, `(BF − champion) / (BF − L1)`. Un champion invalide sur la validation
compte pour −100 %.

**Test principal (H1, à appels égaux).** Évolution 340 contre Tirage 340, test de Wilcoxon
signé apparié, bilatéral, exact, sur les 24 seeds. H1 est établie si p < 0,05 et si
l'évolution gagne plus de seeds qu'elle n'en perd.

**Test secondaire (H2, à dépense égale).** Évolution 340 contre Tirage 455, même test, même
seuil. H2 n'est interprétée que si H1 est établie.

**Test d'échelle (H3).** Pour chaque seed, l'écart est l'effet de l'évolution moins celui
du tirage à appels égaux, sur la validation. Les 24 écarts à 340 appels (ce protocole) sont
comparés aux 24 écarts à 85 appels (protocole de confirmation, seeds 6000 à 6391) par un
test de Mann-Whitney bilatéral (approximation normale, corrections de continuité et d'ex
aequo). H3 est établie si p < 0,05 et si l'écart médian à 340 appels est le plus grand.

**Lecture.**

- H1 établie : l'avantage de l'évolution à appels égaux tient à un budget quatre fois plus
  grand.
- H1 et H2 établies : il tient aussi à dépense égale.
- H1 non établie : l'avantage observé à 85 appels n'est pas retrouvé à 340.
- Test significatif en faveur d'un tirage : l'évolution fait moins bien que le tirage à ce
  budget.
- H3 établie : l'écart grandit avec le budget. H3 non établie : ce protocole ne montre pas
  que l'écart grandit ; il ne montre pas non plus qu'il ne grandit pas (voir limites).
- H3 significatif dans l'autre sens : l'écart se réduit quand le budget augmente.

**Mesures descriptives, sans valeur de test.** Par bras : nombre de seeds à 10 % ou plus et
à 30 % ou plus sur la validation ; effet médian ; mêmes mesures sur OOD-1 et OOD-2 ;
champions invalides ; nombre de champions de l'évolution qui sont des propositions de
départ non modifiées ; tokens et coût par bras.

## Limites connues

- Un seul modèle, un seul problème.
- H3 compare deux expériences faites à un jour d'écart, sur des seeds différentes, avec
  deux exécutables (différence limitée au client HTTP). Un modèle distant peut être modifié
  par son fournisseur entre-temps. Avec 24 écarts de chaque côté, très dispersés, ce test
  ne peut voir qu'un changement important.
- Sans champ `seed`, les réponses du modèle ne sont pas reproductibles.
- Le budget à dépense égale repose sur trois runs d'essai ; le rapport de coût réel peut
  s'en écarter.
- Multiplier par quatre les deux parts de l'évolution est un choix parmi d'autres : un
  départ plus court suivi de plus de générations n'est pas testé.
- Les propositions fraîches reçoivent un des cinq « archétypes » du prompt de génération,
  dont un proche de l'astuce de la référence manuelle. Le prompt de mutation n'en contient
  pas.
- L'interpréteur corrigé garde des tolérances documentées : un champion peut ne pas être du
  Rust compilable.
- Les trois modifications du moteur (départ large, relance, mutation informée) sont testées
  ensemble.
