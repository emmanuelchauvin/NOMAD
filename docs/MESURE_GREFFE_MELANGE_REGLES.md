# Mesure hors ligne : greffe avec mélanges contre greffe de règles complètes

Mesure du 16 au 17 novembre 2026 sur la tour, hors protocole, sans appel au modèle.

**Note de méthode :** les données brutes ont été lues et dépouillées *avant* d'être
commitées, contrairement à la règle du projet. Le banc et ses paramètres ont été fixés et
commités avant l'exécution (commit 9228c13). En revanche, l'analyse ci-dessous a été écrite
après avoir vu les chiffres.

## Ce qui est comparé

Il s'agit de deux façons de greffer une unité du programme donneur dans le programme hôte
(`crates/nomad-genome/src/crossover.rs`, `GraftMode`) :

- **Mélanges (`Mixing`)** : c'est le comportement historique. Une définition du donneur
  que l'hôte ne lit pas devient un mélange `score = score * 0.95 + e * 0.05`.
- **Règles complètes (`RulesOnly`)** : seules les instructions du donneur qui modifient
  `score` sont greffées, avec leur tranche de dépendances. Il n'y a jamais de mélange.

## Données et banc

- **Paires :** tous les croisements des 12 tirs GLM locaux
  (`results_glm_local/local/run_seed_<seed>_local.json`, seeds 9600 à 9787 par pas de 17),
  soit 138 par seed et **1 656 paires** hôte/donneur. L'hôte est `parent_ids[0]` et le
  donneur `parent_ids[1]`.
- **Banc :** `crates/nomad-cli/examples/graft_mode_probe.rs`. Chaque paire est rejouée dans
  les deux modes avec la même graine de croisement et le même choix d'opérateur que le
  moteur (opérateur multi-locus ou déterministe selon la parité de la graine).
- **Critère :** comme dans le moteur, l'enfant est gardé s'il est valide et bat son hôte sur
  l'entraînement. Les enfants gardés sont aussi notés sur la validation, avec leur hôte.
- **Réglages :** `NOMAD_BP_PROFILE=weibull`, `NOMAD_FUEL_LIMIT=400000`. Le banc a tourné en
  6 processus de 2 seeds chacun, de 21h20 à 0h56.
- **Fichiers :** les données brutes sont dans `mesures/graft_mode/part{1..6}.jsonl` (une
  ligne par paire) et les temps par seed dans `part*.log`. L'essai préalable sur 10 paires
  est `mesures/graft_mode_essai.jsonl`.
- **Dépouillement :** `python scripts/analysis/graft_mode.py mesures/graft_mode`.

## Résultats

| | Mélanges | Règles complètes |
|---|---|---|
| Greffes impossibles | 0 | 2 |
| Enfants valides | 1 631 | 1 607 |
| Enfants qui battent leur hôte | 237 (14,3 %) | 252 (15,2 %) |
| … dont enfants contenant un mélange | 110 | 0 |
| Gain médian sur l'hôte (entraînement) | 5 bacs | 8 bacs |
| Gain total (entraînement) | 8 187 bacs | 11 226 bacs |
| Gain total sans les gains > 100 bacs | 2 108 bacs | 2 936 bacs |
| Validation : le gain tient | 160 (68 %) | 201 (80 %) |
| Validation : égalité | 27 | 14 |
| Validation : l'enfant fait pire que l'hôte | 50 | 37 |
| Gain médian en validation | 4 bacs | 8 bacs |
| Enfants meilleurs que le champion de leur seed | 5 | 6 |
| Taille ajoutée au code (moyenne, tous enfants) | +40 caractères | +223 caractères |

Tous les hôtes rejoués sont valides.

Comparaison appariée :

- **Paires qui réussissent dans un seul mode :** 157 en règles complètes seulement, 142 en
  mélanges seulement. Sur les 95 paires réussies dans les deux modes, les règles complètes
  donnent un meilleur enfant 48 fois et les mélanges 32 fois.
- **Par seed, sur le nombre d'enfants gardés :** les règles complètes l'emportent sur 7
  seeds, les mélanges sur 4, et il y a 1 égalité.

## Lecture

- **Le nombre d'enfants gardés est indiscernable.** L'écart de 157 contre 142 entre les
  paires réussies dans un seul mode est compatible avec le hasard.
- **La qualité des gains favorise les règles complètes.** Le gain médian est plus élevé, il
  tient plus souvent en validation (80 % contre 68 %) et les enfants qui régressent en
  validation sont moins nombreux. L'avantage reste quand on retire les quelques gains
  supérieurs à 100 bacs.
- **Le mode historique ne réussit pas grâce aux mélanges.** Ils sont présents dans 861
  enfants, mais seulement dans 110 des 237 enfants gardés. Les autres réussites du mode
  historique sont des greffes de règles.
- **Le coût des règles complètes est la taille du code.** Leurs enfants grossissent environ
  5 fois plus. Avec ce mode, l'élagage du champion devient d'autant plus utile.

## Limites

- Les paires viennent de tirs passés, avec un seul modèle (GLM local) et un seul profil
  (Weibull).
- Un enfant n'est évalué qu'une fois ; on ne voit pas son effet sur la suite d'une évolution.
- Aucun test statistique n'a été fixé à l'avance. Ces chiffres orientent le choix du mode à
  tester dans un vrai tir ; ils ne le prouvent pas.
