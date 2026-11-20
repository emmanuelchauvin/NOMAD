# Protocole « tirage contre évolution corrigée » sur MiniMax (fixé avant exécution)

Rédigé le 13 novembre 2026, avant tout run de ce protocole. Les critères ci-dessous ne
doivent pas être modifiés après lecture des résultats.

## Question

À budget égal d'appels au modèle, l'évolution de NOMAD, une fois sa sélection corrigée,
découvre-t-elle une bonne heuristique plus souvent qu'un tirage de propositions
indépendantes ?

## Contexte

Le témoin du 12 novembre ([résultats](RESULTATS_WEIBULL_TEMOIN.md)) a montré qu'avec
`gpt-oss:20b` le tirage réussit sur 7 seeds sur 12, contre 4 pour chaque groupe évolutif.
L'examen du moteur a ensuite trouvé deux défauts, corrigés depuis (commit `759e802`) :

- le parent d'une mutation était choisi presque uniformément entre les niches : 74 à 80 %
  des mutations partaient de parents pires que Best-Fit. Il est maintenant tiré par rang ;
- la grille de niches d'un run de bin packing utilisait les formules de l'ordonnancement.

Ce protocole change aussi de modèle, pour passer d'un calcul local à une API. Ses
résultats ne sont donc pas comparables à ceux des campagnes v1, v2 et du premier témoin.

## Dispositif commun aux deux bras

- Benchmark : profil Weibull inchangé (`NOMAD_BP_PROFILE=weibull`), mêmes quatre jeux
  (entraînement, validation, OOD-1, OOD-2), fuel `NOMAD_FUEL_LIMIT=400000` mis à l'échelle.
- Modèle : `MiniMax-M3` par l'API `https://api.minimax.io/v1/chat/completions`,
  raisonnement désactivé (`{"thinking":{"type":"disabled"}}`), température 0,7, top_p 0,95.
- 12 seeds : 4000, 4017, ..., 4187. Aucune n'a servi à un essai.
- Budget : 85 appels au modèle par run.
- Code figé par le tag git `minimax-v1`, exécutable `target/frozen/nomad-minimax-v1.exe`.
- Lancement par `scripts/campaign/run_minimax_arm.ps1`, une seed après l'autre. Le script
  saute les seeds terminées et enregistre les tokens de chaque appel.

## Les deux bras

| Bras | Procédé | Commande |
|---|---|---|
| Tirage | 85 propositions fraîches, la meilleure à l'entraînement est le champion | `run -G D --pop-size 85 -n 1` |
| Évolution | Groupe D (mutation seule), population 8, 12 générations, sélection par rang | `run -G D --pop-size 8 -n 12` |

Le bras tirage est exécuté en premier. Le bras évolution n'est exécuté que si le tirage
montre que le modèle convient (voir « Condition préalable »).

Le groupe C (croisement) n'est pas dans ce protocole : la question porte sur la sélection.

## Mesures et lecture, fixées à l'avance

**Mesure principale.** Par bras, nombre de seeds où le champion supprime au moins 10 % de
l'excès de Best-Fit sur la validation, `(BF − champion) / (BF − L1)`.

**Mesures secondaires.** Effet médian sur la validation ; effets sur OOD-1 et OOD-2 ;
nombre de seeds où l'évolution fait mieux que le tirage sur la validation ; test de
Wilcoxon signé apparié, bilatéral, entre les deux bras sur la validation.

**Condition préalable.** Si le tirage réussit sur 0 ou 1 seed sur 12, le modèle dans ce
réglage est jugé trop faible pour ce protocole : le bras évolution n'est pas exécuté, et
un nouveau protocole est écrit avec un autre réglage.

**Lecture de la comparaison.**

- Évolution à au moins 3 succès de plus que le tirage : indice en faveur de l'évolution
  corrigée, à confirmer sur plus de seeds.
- Écart de 2 succès ou moins, dans un sens ou dans l'autre : pas de différence montrée à
  ce budget.
- Tirage à au moins 3 succès de plus que l'évolution : l'évolution corrigée reste moins
  bonne que le tirage à ce budget.

## Limites connues

- 12 seeds : seule une différence très nette serait statistiquement établie. Ce protocole
  oriente la suite, il ne clôt pas la question.
- Un seul budget (85 appels). Il ne dit rien de budgets plus grands.
- Les propositions fraîches reçoivent un des cinq « archétypes » du prompt de génération,
  dont un proche de l'astuce de la référence manuelle. Le prompt de mutation n'en contient
  pas. Les deux bras comparent des procédés complets, pas des prompts.
- Un modèle distant peut être modifié par son fournisseur. La date et le nom du modèle
  sont notés ; la reproduction exacte n'est pas garantie.
- La seed 3000 a servi aux essais de connexion et n'est pas utilisée.
