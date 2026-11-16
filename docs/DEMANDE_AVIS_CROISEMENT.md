# Demande d'avis : le croisement AST de NOMAD et la grille en îles

Ce fichier ouvre un extrait du projet NOMAD. Il dit ce que nous attendons de vous ; le reste
de l'extrait est le code et les documents utiles pour y répondre. Soyez critiques : un avis
complaisant ne nous sert à rien.

## Le projet en bref

NOMAD fait évoluer des heuristiques écrites en Rust par un LLM, ici pour le bin packing en
ligne (profil Weibull, 10 instances de 1 000 objets, capacité 100). Chaque programme est une
fonction `score_bin` qui note chaque bac candidat ; l'objet va dans le bac de score minimal.
Les programmes sont exécutés par un interpréteur AST borné en fuel : l'évaluation est
déterministe et rejouable sur n'importe quelle machine, sans horloge.

L'objectif à terme : une grille de machines volontaires sur Internet, organisées en îles,
qui échangent du matériel génétique ([GRID_ARCHITECTURE.md](GRID_ARCHITECTURE.md)).

## Ce qui est établi

Protocoles fixés avant exécution, tests de Wilcoxon appariés, sur `gemini-3.1-flash-lite`
([RESULTATS_GEMINI_COURBE.md](RESULTATS_GEMINI_COURBE.md)) :

- H1 : à appels égaux, l'évolution (mutation par le LLM, sélection MAP-Elites) bat un tirage
  de propositions indépendantes du même modèle.
- H2 : à dépense égale aussi.
- H3 : l'avance grandit entre 340 et 1 359 appels.

Ces résultats n'utilisent **pas** le croisement. Une réplication sur un second modèle
(`glm-5.3-flash`) est en cours.

## Ce qui est en question : le croisement AST

Le croisement (`crates/nomad-genome/src/crossover.rs`) greffe une instruction de premier
niveau d'un programme donneur dans un programme hôte, sans appel au LLM. Le contrôle
d'ablation (`crates/nomad-evaluator/src/anti_dead_code.rs`, niveau 3) retire ensuite la
greffe et vérifie que l'enfant se dégrade sans elle.

Corrigé le 15 novembre 2026 (branche `ast-croisement`) :

- les sondes du niveau 3 utilisaient une instance du jeu OOD-2 (fuite vers la sélection) ;
- un enfant rejeté pouvait devenir le champion mesuré ;
- l'analyse de dépendances refusait à tort 18 % des greffes (noms de méthode comme `powi`
  pris pour des variables, `let` internes à la règle) ;
- un fragment non résolu était inséré tel quel.

Défauts connus, non corrigés :

- une variable intermédiaire du donneur devient `score * 0.95 + x * 0.05` (constantes
  arbitraires) ;
- la greffe n'ajoute que : les programmes grossissent ;
- capture de nom : une règle du donneur qui lit `progress` lit le `progress` de l'hôte ;
- le niveau 3 accepte un enfant moins bon que son hôte, pourvu que la greffe serve dans
  l'enfant.

Mesures hors ligne, provisoires (ancien code, 112 enfants sur les 3 meilleures élites de 7
runs Gemini) :

| | Intra-île (56) | Inter-îles (56) |
|---|---|---|
| Jugés nocifs par ablation | 12 | 24 |
| Battent leur hôte | 4 (3 confirmés en validation) | 2 (1 confirmé) |
| Battent le champion du run | 1 (−2 bacs) | 1 (−2 bacs) |

Pour comparaison, une mutation par le LLM partant du champion du moment l'améliore 1,8 % du
temps (0,9 % dans le dernier tiers des runs).

## Notre piste

Utiliser l'ablation non seulement pour valider une greffe, mais pour mesurer l'effet de
chaque règle de chaque élite. Trois usages :

1. élaguer les règles sans effet (contre le grossissement et le coût par appel) ;
2. greffer les règles d'effet mesuré, et remplacer au lieu d'empiler ;
3. entre îles, échanger des gènes (une règle, ses dépendances, son effet mesuré) plutôt que
   des programmes, chaque île revérifiant par ablation sur ses propres instances.

Les volontaires apporteraient du calcul (ablations, greffes, vérifications) ; les appels
payants au LLM seraient réservés à l'invention.

## Nos questions

1. Le croisement par greffe d'instructions est-il une bonne unité de recombinaison pour des
   programmes de ce type (sommes de règles additives) ? Que proposeriez-vous à la place ou en
   plus ?
2. Les défauts listés sont-ils les bons ? En voyez-vous d'autres dans le code fourni ?
3. La piste « gènes mesurés par ablation » vous paraît-elle solide ? Où risque-t-elle de
   tromper (interactions entre règles, sur-apprentissage aux instances d'entraînement) ?
4. Quel travail publié est le plus proche (FunSearch, EoH, ReEvo, autres) et en quoi
   NOMAD s'en distingue-t-il, ou non ?
5. Pour prétendre à l'état de l'art en bin packing en ligne, quelles instances et quelle
   mesure faut-il reprendre ? Nos programmes voient la taille de l'objet suivant : est-ce
   rédhibitoire pour une comparaison ?
6. Quelle expérience feriez-vous en premier, avec un budget d'environ 30 $ d'appels ?

Merci de distinguer ce que vous affirmez après lecture du code de ce que vous supposez.
