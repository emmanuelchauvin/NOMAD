# Protocole à 344 appels sur un second modèle, glm-5.3-flash (fixé avant exécution)

Rédigé le 15 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

Il remplace [PROTOCOLE_GLM_COURBE.md](PROTOCOLE_GLM_COURBE.md), abandonné après son essai
préalable : le bras d'évolution était trop lent à 1 359 appels.

## Question

L'avantage de l'évolution sur le tirage, obtenu sur `gemini-3.1-flash-lite`
([RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md)), se retrouve-t-il avec un modèle
d'un autre fabricant ?

- H1 : à appels égaux, l'évolution fait mieux que le tirage.
- H2 : à dépense égale, aussi.

H3 (l'avance grandit avec le budget) n'est pas testée ici : elle demande un budget plus
grand et fera l'objet d'un autre protocole, sur un autre modèle.

## Dispositif

Celui de [PROTOCOLE_GLM_COURBE.md](PROTOCOLE_GLM_COURBE.md), au budget près.

- **Modèle.** `z-ai/glm-5.3-flash` par OpenRouter, hébergeur unique Relace (fp4) sans repli,
  raisonnement au minimum, pas de champ `seed`, température 0,7 et top_p 0,95.
- **Exécutable.** `target/frozen/nomad-gemini-courbe.exe` (tag git `gemini-courbe`,
  SHA-256 `1f16312c10511e5407bf316a1aa12be32f05d6f4549dc35e946f9dbe66ce4c22`) : même moteur,
  mêmes prompts, même interpréteur que sur Gemini.
- **Benchmark.** Profil Weibull, fuel 400 000.
- **Seeds.** 24 seeds neuves : 9000, 9017, ..., 9391. Aucune n'a servi à un essai. Un seul
  regard, aucune extension.
- **Budget : 344 appels par run, pour les deux bras.** Le moteur ne s'arrête qu'en fin de
  génération : 22 + 46 × 7 = 344. Le tirage reçoit le même nombre de propositions, pour que
  la comparaison soit exactement à appels égaux.

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 344 propositions fraîches | `run -G D --pop-size 344 -n 1` |
| Évolution | Groupe D, 22 propositions fraîches puis 46 générations de 7 appels (344 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée | `run -G D --pop-size 8 -n 47`, `NOMAD_INIT_POP=22` |

- **Lancement.** `scripts/campaign/lancer_glm_340.ps1` : les deux bras en parallèle, huit
  processus par bras, trois seeds chacun. Le découpage ne change pas les résultats.

## Mesures et lecture, fixées à l'avance

**Paliers.** 85, 170 et 344 appels.

**Grandeur mesurée.** Pour une seed, un bras et un palier `k` : l'effet sur la validation
du champion après `k` propositions (dernière proposition marquée `is_new_best` parmi les
`k` premières). Effet : part de l'excès de Best-Fit supprimée, `(BF − champion) / (BF − L1)`.
Un champion invalide sur la validation compte pour −100 %. Une seed sans aucun champion à
un palier compte pour −100 % à ce palier.

**Tests.** Wilcoxon signé apparié, bilatéral, exact, sur les 24 seeds, seuil 0,05.

- H1 : évolution contre tirage à 344 appels. Établie si p < 0,05 et si l'évolution gagne
  plus de seeds qu'elle n'en perd.
- H2 : `r` est le coût par appel de l'évolution divisé par celui du tirage, mesuré sur les
  48 runs au tarif de Relace (0,0399 $ par million de tokens en entrée, 0,0784 $ en
  sortie). L'évolution arrêtée à `k* = ⌊344 / r⌋` appels contre le tirage complet. Établie
  si p < 0,05 et si l'évolution gagne plus de seeds qu'elle n'en perd. H2 n'est interprétée
  que si H1 est établie.

**Lecture.**

- H1 et H2 établies : les deux résultats obtenus sur Gemini à 340 appels sont répliqués sur
  un second modèle.
- H1 établie, H2 non : l'évolution fait mieux à appels égaux sur ce modèle, mais pas à
  dépense égale.
- H1 non établie : l'avantage de l'évolution n'est pas retrouvé sur ce modèle à ce budget.
- Un test significatif dans le sens contraire est rapporté comme tel.

**Mesures descriptives, sans valeur de test.** Médianes et succès (≥ 10 %, ≥ 30 %) par
palier sur la validation et l'entraînement, écart par palier, budget de tirage équivalent,
dernier appel améliorant, part des champions issus d'une relance, part de propositions
invalides par bras, tokens et coût.

**Dépouillement.** `python scripts/analysis/gemini_courbe.py --glm340` : les calculs du
protocole de courbe, avec le dossier, les seeds, le budget, les paliers et les prix de ce
protocole, sans le test H3. Les données brutes sont commitées avant d'être lues, et le
script est d'abord passé en mode `controle`.

## Ce que l'essai préalable laisse attendre

L'essai préalable de [PROTOCOLE_GLM_COURBE.md](PROTOCOLE_GLM_COURBE.md) (seeds 3000 à 3085,
hors protocole) couvre ce budget ; il n'est pas refait. Mesures sur ses 340 premiers appels :

- **Coût.** Environ 0,028 $ par run de tirage et 0,07 $ par run d'évolution, soit environ
  2,40 $ pour les 48 runs.
- **Rapport de coût.** `r` voisin de 2,5 (1,5 sur Gemini) : les programmes grossissent au
  fil des mutations. `k*` sera donc voisin de 135 appels. H2 est un test plus dur que sur
  Gemini.
- **Tirage plus fort que sur Gemini.** Champions de 9 % à 49 % à 680 propositions, contre
  une médiane de 17,5 % sur Gemini. H1 n'est pas acquise.
- **Durée.** L'appel au modèle ne compte que pour 27 à 38 % du temps écoulé ; le reste est
  l'évaluation locale des programmes, qui utilise tous les cœurs. Sur le PC de l'essai
  (6 cœurs, 12 processus), les deux runs d'évolution les plus rapides ont atteint 340 appels
  en 2 h 20 ; les quatre autres n'en avaient fait que 150 à 264 en 2 h 40. Sur la machine
  du protocole (16 cœurs), durée estimée, non mesurée : 5 à 8 h pour l'ensemble.

Un essai de fumée (`lancer_glm_340.ps1 -Essai` : seed 3102, 29 appels par bras, moins de
0,01 $) vérifie la clé, le réseau et l'exécutable sur la machine avant le lancement. Ses
scores ne sont pas lus.

## Limites connues

- Un seul problème ; deux modèles avec celui-ci.
- Le budget est de 344 appels, non 340 : la comparaison avec le palier 340 de Gemini est
  approchée.
- Le modèle est servi par un hébergeur tiers en version quantifiée (fp4).
- Un hébergeur unique peut limiter le débit ou tomber en panne : le client rejoue six fois
  un appel en échec, et un run en échec est relancé trois fois.
- Le réglage du moteur (départ de 22, relance, mutation informée) a été choisi sur Gemini
  et n'est pas réajusté pour ce modèle.
- Réponses non reproductibles, faute de champ `seed`.
