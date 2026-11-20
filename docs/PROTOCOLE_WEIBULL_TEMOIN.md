# Protocole du témoin « tirage sans évolution » (fixé avant exécution)

Rédigé le 12 novembre 2026, après la campagne v2 ([résultats](RESULTATS_WEIBULL_V2.md))
et avant le lancement du témoin. Les critères ci-dessous ne doivent pas être modifiés
après lecture des résultats.

## Question

À budget égal d'appels au modèle, l'évolution de NOMAD découvre-t-elle une bonne
heuristique plus souvent qu'un simple tirage de propositions indépendantes ?

## Pourquoi ce témoin

Le dépouillement des 42 runs des campagnes v1 et v2 montre que :

- la découverte (plus de 10 % de l'excès de Best-Fit supprimé à l'entraînement) survient
  dans 13 runs sur 42, à une génération quelconque entre 1 et 9 ;
- la population de départ (8 propositions fraîches par run) contient déjà la découverte
  dans 2 runs sur 42, soit 2 succès sur 336 propositions fraîches ;
- les générations suivantes produisent 11 découvertes pour environ 2 500 appels.

Les deux taux par appel sont du même ordre (environ 0,5 %). Ces chiffres sont trop petits
pour conclure. Ce témoin mesure directement le tirage sans évolution.

## Dispositif

- Même exécutable figé que la v2 (`target/frozen/nomad-campaign-weibull-v2.exe`, tag
  `campaign-weibull-v2`), même modèle (`gpt-oss:20b`, Ollama 0.40.16), même profil
  (`NOMAD_BP_PROFILE=weibull`), même fuel (`NOMAD_FUEL_LIMIT=400000`, mis à l'échelle).
- Mêmes 12 seeds que la v2 (2000, 2017, ..., 2187), donc mêmes jeux de données.
- Par seed : 85 propositions fraîches du modèle (population 85, 1 génération), sans
  mutation, sans croisement, sans sélection intermédiaire. Le champion est la meilleure
  des 85 sur l'entraînement. Les runs de la v2 consommaient eux aussi 85 appels.
- Commande, par seed : `run -G D --seed <seed> --pop-size 85 -n 1 --operator ollama
  --model gpt-oss:20b --output results_weibull_temoin/run_seed_<seed>_tirage.json`.
- Lancement par `scripts/campaign/run_temoin_tirage.ps1`, qui saute les seeds déjà
  terminées (reprise possible après une interruption).

## Mesures et lecture, fixées à l'avance

**Mesure principale.** Nombre de seeds où le champion supprime au moins 10 % de l'excès
de Best-Fit sur la validation. Références de la v2 sur les mêmes seeds : groupe C 4 sur
12, groupe D 4 sur 12.

**Mesure secondaire.** Effet médian sur la validation, et effets sur OOD-1 et OOD-2.

**Lecture.**

- Témoin à 3 succès ou plus sur 12 : à ce budget, l'évolution ne découvre pas plus
  souvent que le tirage. La découverte est attribuée au modèle, pas à NOMAD.
- Témoin à 0 ou 1 succès sur 12 : indice que l'évolution aide à la découverte, à
  confirmer sur plus de seeds.
- Témoin à 2 succès sur 12 : indécis.

## Limites connues

- 12 seeds : ce témoin ne permet aucun test significatif entre le tirage et les groupes
  C et D. Il donne un ordre de grandeur et oriente la suite.
- Les propositions fraîches utilisent le prompt de génération initiale, les générations
  suivantes un prompt de mutation. Le témoin compare donc deux procédés complets, pas
  deux prompts.
- Les résultats de la v2 sont connus au moment où ce protocole est écrit.
