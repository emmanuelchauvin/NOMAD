# Protocole de courbe d'échelle sur un second modèle, glm-5.3-flash (fixé avant exécution)

> **Non exécuté : abandonné après l'essai préalable** (voir la section « Résultat de l'essai
> préalable » en fin de document). Le bras d'évolution est trop lent à 1 359 appels sur ce
> modèle. Il sera remplacé par un protocole à 340 appels, à écrire et commiter avant tout
> lancement.

Rédigé le 15 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

Les résultats obtenus sur `gemini-3.1-flash-lite`
([RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md)) se retrouvent-ils avec un modèle
d'un autre fabricant ?

- H1 : à appels égaux, l'évolution fait mieux que le tirage.
- H2 : à dépense égale, aussi.
- H3 : l'avance de l'évolution est plus grande à 1 359 appels qu'à 340.

C'est une réplication : le dispositif, les mesures et les tests sont ceux de
[PROTOCOLE_GEMINI_COURBE.md](PROTOCOLE_GEMINI_COURBE.md). Seuls changent le modèle, les
seeds et le nombre de seeds.

## Ce qui change

- **Modèle.** `z-ai/glm-5.3-flash`, appelé par OpenRouter
  (`https://openrouter.ai/api/v1/chat/completions`).
- **Hébergeur unique.** OpenRouter répartit ce modèle entre une quarantaine d'hébergeurs,
  à des prix et des niveaux de quantification différents. Tous les appels des deux bras
  sont envoyés au même hébergeur (Relace, quantification fp4, le moins cher), sans repli
  sur un autre : `"provider":{"order":["Relace"],"allow_fallbacks":false}`.
- **Raisonnement au minimum.** `"reasoning":{"effort":"minimal"}`. Il ne peut pas être
  coupé sur ce modèle. Sans ce réglage, une réponse contient environ 3 000 tokens de
  raisonnement ; avec lui, moins de 30 (quatre appels d'essai).
- **Pas de champ `seed`**, comme sur Gemini. Température 0,7 et top_p 0,95, inchangés.
- **24 seeds neuves d'emblée** : 9000, 9017, ..., 9391. Aucune n'a servi à un essai. Le
  coût par appel, environ dix fois plus bas que sur Gemini, permet de se passer du schéma
  en deux regards : un seul test, sur 24 seeds, au seuil de 0,05. Aucune extension.

## Ce qui ne change pas

- Exécutable `target/frozen/nomad-gemini-courbe.exe` (tag git `gemini-courbe`) : même
  moteur, mêmes prompts, même interpréteur.
- Benchmark : profil Weibull, fuel 400 000.
- Les deux bras :

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 1 359 propositions fraîches | `run -G D --pop-size 1359 -n 1` |
| Évolution | Groupe D, 22 propositions fraîches puis 191 générations de 7 appels (1 359 appels), sélection par rang, relance après 3 générations sans progrès, mutation informée | `run -G D --pop-size 8 -n 192`, `NOMAD_INIT_POP=22` |

- Lancement par `scripts/campaign/lancer_glm_courbe.ps1` : les deux bras en parallèle,
  six processus par bras, quatre seeds chacun.

## Mesures et lecture, fixées à l'avance

Identiques à celles du protocole Gemini.

**Paliers.** 85, 170, 340, 680 et 1 359 appels.

**Grandeur mesurée.** Pour une seed, un bras et un palier `k` : l'effet sur la validation
du champion après `k` propositions (dernière proposition marquée `is_new_best` parmi les
`k` premières). Effet : part de l'excès de Best-Fit supprimée, `(BF − champion) / (BF − L1)`.
Un champion invalide sur la validation compte pour −100 %. Une seed sans aucun champion à
un palier compte pour −100 % à ce palier.

**Écart.** Pour une seed et un palier : effet de l'évolution moins effet du tirage.

**Tests.** Wilcoxon signé apparié, bilatéral, exact, sur les 24 seeds, seuil 0,05.

- H1 : évolution contre tirage à 1 359 appels. Établie si p < 0,05 et si l'évolution gagne
  plus de seeds qu'elle n'en perd.
- H2 : `r` est le coût par appel de l'évolution divisé par celui du tirage, mesuré sur les
  48 runs. L'évolution arrêtée à `k* = ⌊1 359 / r⌋` appels contre le tirage complet. H2
  n'est interprétée que si H1 est établie.
- H3 : écart à 1 359 appels contre écart à 340 appels. Établie si p < 0,05 et si l'écart
  grandit sur plus de seeds qu'il ne se réduit.

**Lecture.**

- H1, H2 et H3 établies : les trois résultats obtenus sur Gemini sont répliqués sur un
  second modèle.
- H1 établie, H3 non : l'avantage de l'évolution est répliqué, pas l'effet d'échelle.
- H1 non établie : l'avantage de l'évolution n'est pas retrouvé sur ce modèle.
- Un test significatif dans le sens contraire est rapporté comme tel.

**Mesures descriptives, sans valeur de test.** Celles du protocole Gemini (médianes et
succès par palier, écart par palier, budget de tirage équivalent, dernier appel
améliorant, part des champions issus d'une relance, tokens et coût), plus la part de
propositions invalides par bras, pour comparer les deux modèles.

**Dépouillement.** `scripts/analysis/gemini_courbe.py`, auquel sont ajoutés avant le
dépouillement le choix du dossier, des seeds et des prix, sans changer les calculs.

## Essai préalable

Douze runs de 680 appels sur des seeds hors protocole (3000 à 3085, six par bras),
lancés avec douze processus comme le sera le protocole. Données dans
`results_glm_courbe_pilote/`. Ont été regardés : la part de propositions valides, le coût
réel facturé, la durée, les erreurs de l'API, et si le modèle produit des heuristiques
meilleures que Best-Fit (sans quoi le protocole ne pourrait rien mesurer). Résultats :
voir la section ajoutée ci-dessous avant le lancement.

## Limites connues

- Celles du protocole Gemini, sauf « un seul modèle » : il y en a maintenant deux, sur un
  seul problème.
- Le modèle est servi par un hébergeur tiers en version quantifiée (fp4) : ce n'est pas
  exactement le modèle servi par son fabricant.
- Un hébergeur unique peut limiter le débit ou tomber en panne : le client rejoue six fois
  un appel en échec, et un run en échec est relancé trois fois.
- Le réglage du moteur (départ de 22, relance, mutation informée) a été choisi sur Gemini
  et n'est pas réajusté pour ce modèle.

## Résultat de l'essai préalable

Lancé le 15 novembre 2026 à 12 h 34, arrêté à 15 h 15 avant la fin du bras d'évolution.
Données (partielles) dans `results_glm_courbe_pilote/`. Dépense réelle chez OpenRouter :
0,75 $.

**Tirage.** 5 runs sur 6 terminés (680 propositions) ; le sixième (seed 3000) a été arrêté
à quelques appels de la fin.

| Seed | Valides | Meilleures que Best-Fit | Champion, validation |
|---|---|---|---|
| 3017 | 88 % | 14 % | 48,6 % |
| 3034 | 88 % | 12 % | 9,2 % |
| 3051 | 86 % | 20 % | 35,9 % |
| 3068 | 88 % | 13 % | 25,3 % |
| 3085 | 88 % | 13 % | 27,7 % |

Environ 660 tokens en entrée et
760 en sortie par appel, 0,00009 $ par appel, 4,9 s par appel en moyenne. Le modèle convient
à la tâche, et son tirage est plus fort que celui de Gemini (médiane de Gemini à 680
propositions : 17,5 %).

**Évolution.** Aucun run terminé : de 136 à 342 appels sur 680 en 2 h 20. Les programmes
grossissent au fil des mutations : la réponse moyenne passe d'environ 1 100 à 4 700 tokens
de sortie selon les seeds, et le temps d'un cycle (appel et évaluation) de 25 à 77 s, contre
8 à 12 s sur Gemini. Coût par appel : environ 0,00026 $, trois fois celui du tirage, et en
hausse.

**Conséquence.** 24 seeds à 1 359 appels demanderaient plusieurs jours et dépasseraient le
crédit disponible. Ce protocole n'est pas exécuté. Suite prévue : le même dispositif à
340 appels sur 24 seeds (H1 et H2, sans H3), dans un nouveau protocole.
