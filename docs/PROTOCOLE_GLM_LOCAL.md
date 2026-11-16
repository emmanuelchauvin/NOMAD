# Protocole : croisement AST local et élagage dans l'évolution, sur glm-5.3-flash (fixé avant exécution)

Rédigé le 16 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Les protocoles précédents comparent l'évolution à un tirage de propositions indépendantes.
Celui-ci compare deux évolutions : l'une avec les opérateurs locaux de NOMAD, l'autre sans.
Ces opérateurs travaillent sans appel au modèle :

- **le croisement AST local** greffe dans un élite une ou deux instructions d'un autre élite
  de l'archive ;
- **l'élagage** retire au nouveau champion ses instructions sans effet.

- H : à appels au modèle égaux (344), l'évolution avec opérateurs locaux fait mieux que
  l'évolution seule.

## Ce qui motive le test

Mesures hors ligne sur les archives des runs Gemini du protocole de courbe (branche
`ast-croisement`, dossier `mesures/`) :

- **Croisement.** Sur 192 enfants des 3 meilleures élites de 12 runs, avec le croisement
  corrigé, 22 battent leur hôte sur l'entraînement, dont 16 confirmés en validation, et 6
  battent le champion du run, tous confirmés en validation (jusqu'à 80 bacs).
- **Élagage.** Sur les 24 champions, 45 instructions sur 273 sont retirées sans perte de bac
  sur l'entraînement, avec un gain total de 130 bacs en validation.

Le protocole GLM à 344 appels ([RESULTATS_GLM_340.md](RESULTATS_GLM_340.md)) montre aussi
qu'un appel d'évolution coûte 4,4 fois un appel de tirage, à cause de la taille des
programmes. L'élagage pourrait réduire ce coût : c'est mesuré ici, sans test.

## Dispositif

- **Modèle.** `z-ai/glm-5.3-flash` par OpenRouter, mêmes réglages que
  [PROTOCOLE_GLM_340.md](PROTOCOLE_GLM_340.md) : hébergeur unique Relace (fp4) sans repli,
  raisonnement au minimum, pas de champ `seed`, température 0,7, top_p 0,95.
- **Exécutable.** `target/frozen/nomad-local-ops.exe`, compilé depuis la branche
  `ast-croisement`, tag `glm-local` (commit `c253590`), SHA-256
  `3055c7b182145d11f0bcfcd3aa316fafc90266de9934048805999e22bcfb649b`. **Les deux bras
  utilisent le même exécutable** ; seules deux variables d'environnement les distinguent.
- **Benchmark.** Profil Weibull, fuel 400 000.
- **Seeds.** 12 seeds neuves : 9600, 9617, ..., 9787 (9600 + 17 r). Extension prévue :
  9804 à 9991.
- **Budget.** 344 appels au modèle par run : 22 propositions fraîches, puis 46 générations
  de 7 appels. Sélection par rang, relance après 3 générations sans progrès, mutation
  informée, comme le bras d'évolution des protocoles précédents.

| Bras | Différence | Variables |
|---|---|---|
| Témoin | Groupe D seul | `NOMAD_LOCAL_CROSSOVERS=0`, `NOMAD_PRUNE_CHAMPION=0` |
| Local | Groupe D + 3 croisements locaux par génération + élagage de chaque nouveau champion | `NOMAD_LOCAL_CROSSOVERS=3`, `NOMAD_PRUNE_CHAMPION=1` |

**Les opérateurs locaux** (code : `crates/nomad-orchestrator/src/engine.rs`,
`local_crossover` et `prune_best_elite`) :

- **Croisement local.** Après les 7 appels de chaque génération, 3 fois : l'hôte est choisi
  par la sélection par rang, le donneur uniformément parmi les autres élites de l'archive,
  et la greffe est faite par le croisement multi-locus ou à un locus. L'enfant est noté sur
  l'entraînement et n'entre dans l'archive que s'il bat son hôte selon l'ordre complet des
  scores. Pas de contrôle par ablation : un enfant qui bat son hôte ne porte pas de greffe
  morte.
- **Élagage.** À la fin de chaque génération, si le meilleur élite a changé, chacune de
  ses instructions (sauf l'expression rendue) est retirée à son tour, du bas vers le haut, en
  une passe. Un retrait est gardé si le programme reste valide sans perdre de bac sur
  l'entraînement. La version élaguée entre dans l'archive si elle est strictement meilleure
  que l'original.
- **Aucun de ces opérateurs n'appelle le modèle.** Les propositions qu'ils produisent sont
  journalisées (types `crossover` et `prune`) avec le nombre d'appels déjà consommés.
- **Lancement.** `scripts/campaign/lancer_glm_local.ps1` : un processus par seed et par bras,
  24 processus en parallèle.

## Mesures et lecture, fixées à l'avance

**Paliers.** 85, 170 et 344 appels au modèle.

**Grandeur mesurée.** Pour une seed, un bras et un palier `k` : l'effet sur la validation
du champion après `k` appels. Le champion est la dernière proposition marquée `is_new_best`
dont le nombre d'appels déjà consommés est au plus `k` ; les propositions locales comptent
donc au palier de l'appel qui les précède. Effet : part de l'excès de Best-Fit supprimée,
`(BF − champion) / (BF − L1)`. Un champion invalide sur la validation compte pour −100 %.

**Test.** Wilcoxon signé apparié, bilatéral, exact, sur les 12 seeds, seuil 0,05, à 344
appels. H est établie si p < 0,05 et si le bras local gagne plus de seeds qu'il n'en perd.

**Extension.** Si H n'est pas établie, si p < 0,20 et si le bras local gagne plus de seeds
qu'il n'en perd, 12 seeds de plus (9804 à 9991) sont lancées à l'identique, et le test final
porte sur les 24 seeds au seuil de 0,025. Sinon, un seul regard.

**Lecture.**

- H établie : à appels égaux, le croisement et l'élagage locaux améliorent l'évolution sur
  ce modèle. Les deux opérateurs sont testés ensemble ; leur part respective n'est mesurée
  que de façon descriptive.
- H non établie : pas d'amélioration démontrée à ce budget.
- Un test significatif dans le sens contraire est rapporté comme tel.

**Mesures descriptives, sans valeur de test.** Médianes et succès (≥ 10 %, ≥ 30 %) par palier
sur la validation et l'entraînement ; écart par palier ; type de la proposition à l'origine
du champion final et des champions successifs (mutation, fraîche, croisement, élagage) ;
compteurs des opérateurs locaux ; taille du code du champion final ; tokens, coût et
rapport du coût par appel entre les bras.

**Dépouillement.** `python scripts/analysis/glm_local.py` (`--seeds24` pour l'extension).
Les données brutes sont commitées avant d'être lues, et le script est d'abord passé en mode
`controle`.

## Coût et durée attendus

- **Coût.** Le bras d'évolution du protocole GLM à 344 appels a coûté 0,118 $ par run :
  environ 2,90 $ pour les 24 runs, plus moins de 0,05 $ pour l'essai de fumée.
- **Durée.** Estimée, non mesurée : 10 à 14 h. Le bras local fait environ deux fois plus
  d'évaluations de programmes (croisements et élagages), mais l'élagage raccourcit les
  programmes, ce qui accélère les évaluations suivantes.

Un essai de fumée (`lancer_glm_local.ps1 -Essai` : seed 3119, 29 appels par bras) vérifie la
clé, le réseau et l'exécutable avant le lancement. Ses scores ne sont pas lus ; on vérifie
seulement que le bras local a produit des propositions locales et que les deux bras ont fait
29 appels.

## Limites connues

- Un seul problème, un seul modèle ; 12 seeds sans extension.
- Les deux opérateurs sont testés ensemble.
- Les croisements portent sur des élites d'une même archive (intra-île) ; l'échange entre
  îles n'est pas testé ici.
- Le croisement local fait des évaluations supplémentaires sur l'entraînement : à appels
  égaux, le bras local dépense plus de calcul local, qui n'est pas facturé.
- L'exécutable diffère de celui des protocoles précédents (corrections du croisement,
  départage déterministe du meilleur élite) : les chiffres du témoin ne sont pas directement
  comparables à ceux du protocole GLM à 344 appels.
- Réponses du modèle non reproductibles, faute de champ `seed`.
