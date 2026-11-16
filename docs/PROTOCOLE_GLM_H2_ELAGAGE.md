# Protocole : évolution avec élagage contre tirage, à dépense égale, sur glm-5.3-flash (fixé avant exécution)

Rédigé le 16 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Sur glm-5.3-flash, l'évolution bat le tirage de propositions indépendantes à appels égaux,
mais pas à dépense égale ([RESULTATS_GLM_340.md](RESULTATS_GLM_340.md)) : un appel
d'évolution y coûtait 4,36 fois un appel de tirage, parce que les programmes grossissent.
L'élagage du champion divise par deux le coût d'un appel d'évolution, sans perte de qualité
à appels égaux ([RESULTATS_GLM_LOCAL.md](RESULTATS_GLM_LOCAL.md)).

- H1 : à appels égaux (344), l'évolution avec élagage fait mieux que le tirage.
- H2 : à dépense égale, aussi.

## Dispositif

- **Modèle.** `z-ai/glm-5.3-flash` par OpenRouter, mêmes réglages que
  [PROTOCOLE_GLM_340.md](PROTOCOLE_GLM_340.md) : hébergeur unique Relace (fp4) sans repli,
  raisonnement au minimum, pas de champ `seed`, température 0,7, top_p 0,95.
- **Exécutable.** `target/frozen/nomad-h2-elagage.exe`, compilé depuis la branche
  `ast-croisement`, tag `glm-h2-elagage` (commit `7caeeb1`), SHA-256
  `9e6e3098f077973970b48d8f0ff50d5b5822a8f633176f1818c8481420dae99b`. **Les deux bras
  utilisent le même exécutable.**
- **Nouveauté de l'exécutable.** Une réponse du modèle qui ne se lit pas comme du Rust,
  autrefois remplacée sans le dire par un gabarit First-Fit ou Best-Fit noté valide, compte
  maintenant comme **invalide** (2,4 % des propositions GLM dans le protocole à 344 appels).
  Cela touche les deux bras de la même façon.
- **Benchmark.** Profil Weibull, fuel 400 000.
- **Seeds.** 24 seeds neuves : 10000, 10017, ..., 10391 (10000 + 17 r).

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 344 propositions fraîches | `run -G D --pop-size 344 -n 1` |
| Évolution | Groupe D : 22 propositions fraîches puis 46 générations de 7 appels (344 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée, **élagage de chaque nouveau champion** (`NOMAD_PRUNE_CHAMPION=1`), sans croisement local | `run -G D --pop-size 8 -n 47`, `NOMAD_INIT_POP=22` |

L'élagage est celui du protocole GLM local (`prune_best_elite` dans
`crates/nomad-orchestrator/src/engine.rs`) : à la fin de chaque génération, si le meilleur
élite a changé, ses instructions sont retirées une à une, du bas vers le haut, en une
passe ; un retrait est gardé si le programme reste valide sans perdre de bac sur
l'entraînement. Il n'appelle pas le modèle.

**Lancement.** `scripts/campaign/lancer_glm_h2.ps1` : évolution, un processus par seed ;
tirage, huit processus de trois seeds. Le découpage ne change pas les résultats.

## Mesures et lecture, fixées à l'avance

**Paliers.** 85, 170 et 344 appels.

**Grandeur mesurée.** Pour une seed, un bras et un palier `k` : l'effet sur la validation
du champion après `k` appels. Le champion est la dernière proposition marquée `is_new_best`
dont le nombre d'appels déjà consommés est au plus `k` (les élagages, gratuits, comptent au
palier de l'appel qui les précède). Effet : part de l'excès de Best-Fit supprimée,
`(BF − champion) / (BF − L1)`. Un champion invalide sur la validation compte pour −100 %.

**Tests.** Wilcoxon signé apparié, bilatéral, exact, sur les 24 seeds, seuil 0,05.

- H1 : évolution contre tirage à 344 appels. Établie si p < 0,05 et si l'évolution gagne
  plus de seeds qu'elle n'en perd.
- H2 : `r` est le coût par appel de l'évolution divisé par celui du tirage, mesuré sur les
  48 runs au tarif de Relace (0,0399 $ par million de tokens en entrée, 0,0784 $ en
  sortie). L'évolution arrêtée à `k* = ⌊344 / r⌋` appels contre le tirage complet (344
  propositions). Établie si p < 0,05 et si l'évolution gagne plus de seeds qu'elle n'en
  perd. H2 n'est interprétée que si H1 est établie.

**Lecture.**

- H1 et H2 établies : avec l'élagage, l'évolution fait mieux que le tirage sur ce modèle à
  appels égaux et à dépense égale ; l'échec de H2 dans le protocole à 344 appels venait du
  coût par appel.
- H1 établie, H2 non : l'élagage ne suffit pas à rendre l'évolution rentable à ce budget.
- Un test significatif dans le sens contraire est rapporté comme tel.

**Mesures descriptives, sans valeur de test.** Médianes et succès (≥ 10 %, ≥ 30 %) par
palier, sur la validation et l'entraînement ; écart par palier ; excès sur la borne L1 en %
de L1 ; propositions invalides par bras ; taille du code du champion final ; compteurs de
l'élagage ; tokens, coût, `r` et `k*`.

**Dépouillement.** `python scripts/analysis/glm_h2.py`. Les données brutes sont commitées
avant d'être lues, et le script est d'abord passé en mode `controle`.

## Coût et durée attendus

- **Coût.** D'après le protocole GLM local : environ 0,059 $ par run d'évolution avec
  élagage et 0,027 $ par run de tirage, soit environ 2,10 $. Si l'essai de fumée projette
  plus de 3 $, le lancement attend l'accord de l'utilisateur.
- **Rapport de coût attendu.** `r` voisin de 2,2, donc `k*` voisin de 156 : c'est là
  qu'H2 se joue. Dans le protocole à 344 appels, l'évolution rattrapait le tirage entre 85
  et 170 appels.
- **Durée.** Estimée, non mesurée : 8 à 12 h, sur le laptop.

Un essai de fumée (`lancer_glm_h2.ps1 -Essai` : seed 3136, 29 appels par bras) vérifie la
clé, le réseau et l'exécutable. Ses scores ne sont pas lus ; on vérifie seulement que les
deux bras ont fait 29 appels et que l'évolution a tenté au moins un élagage.

## Limites connues

- Un seul problème, un seul modèle servi en fp4 ; réponses non reproductibles.
- L'exécutable diffère de celui du protocole à 344 appels (élagage, croisement corrigé,
  gabarits invalides) : les chiffres ne sont pas directement comparables entre les deux
  protocoles.
- Le réglage du moteur a été choisi sur Gemini.
- L'élagage fait des évaluations supplémentaires sur l'entraînement, non facturées.
