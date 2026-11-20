# Correction de l'interpréteur et effet sur les résultats publiés

Rédigé le 13 novembre 2026, après une relecture externe du code.

## Le défaut

Les heuristiques proposées par le modèle sont du code Rust. NOMAD ne les compile pas : il
les exécute avec son propre interpréteur (`crates/nomad-sandbox/src/eval_ast.rs`). Jusqu'au
13 novembre 2026, cet interpréteur s'écartait de Rust, en silence, sur les points suivants :

| Écart | Avant | Après correction |
|---|---|---|
| Variable jamais définie | vaut 0.0 | erreur, le candidat est invalide |
| `let` dans un bloc `else { }` ou un bloc nu `{ }` | ignoré : la variable vaut 0.0 ou garde une ancienne valeur | pris en compte |
| `let x: f64 = ...` (avec annotation de type) | `x` n'est pas définie | définie |
| `const X: f64 = ...;` dans la fonction | ignoré | pris en compte |
| Portée d'un `let` de bloc | la variable survit au bloc | restaurée à la sortie du bloc |
| Constantes à chemin (`f64::MAX`, `f64::INFINITY`) | valent 0.0 | valeur de Rust ; chemin inconnu : erreur |
| Arithmétique entière | tout est flottant : `25 / 20` vaut 1,25 | entiers typés : division tronquée, conversions `as` de Rust |
| Dépassement entier, non signé négatif | résultat flottant négatif | erreur, comme la panique de Rust en mode debug |
| `&&`, `\|\|` | les deux opérandes sont toujours évalués | l'opérande droit n'est évalué que si nécessaire |
| `==`, `!=` | tolérance de 1e-6 | exacts |
| Valeur de la fonction | dernière instruction sans point-virgule, où qu'elle soit | expression finale seulement |
| Instruction non reconnue | ignorée | erreur |

Tolérances conservées, connues : un calcul qui mélange entier et flottant (refusé par Rust)
est fait en flottant ; une division flottante par zéro ou un résultat non fini est une
erreur (Rust renvoie l'infini ou NaN) ; `return` n'est pas reconnu (erreur) ; une fonction
sans expression finale renvoie sa variable `score`.

## Vérification contre le compilateur Rust

`scripts/analysis/diff_interpreter_vs_rustc.py` compile le code de chaque champion avec
`rustc` (1.93.1, mode debug) et compare son résultat à celui de l'interpréteur corrigé sur
400 contextes tirés au hasard.

- 85 champions sur 102 sont du Rust valide. Sur ces 85, l'interpréteur corrigé donne le
  même résultat que Rust dans les 400 contextes.
- 17 champions sont refusés par `rustc` : 13 pour des types incompatibles, 3 pour une
  opération que leurs types ne permettent pas, 1 pour une double affectation d'une variable
  non mutable. L'interpréteur les accepte encore, par les tolérances ci-dessus.

## Effet sur les champions des campagnes

Analyse statique du code des 102 champions (Weibull v1 et v2, témoin, MiniMax v1 et v2) :
aucun n'utilise une variable jamais définie ; 35 ont au moins un `let` dans un bloc `else`.

`scripts/analysis/reeval_champions_interpreter.py` réévalue le même code avec l'exécutable
figé `nomad-minimax-v2.exe` (ancien interpréteur) et avec l'interpréteur corrigé, sur les
mêmes jeux et avec le même fuel. Les scores de Best-Fit sont identiques.

**32 champions sur 102 changent de score sur au moins un jeu.** Sur la validation :

| Campagne | Runs | Changés | Succès ≥ 10 % (avant → après) | Succès ≥ 30 % (avant → après) | Effet médian (avant → après) |
|---|---|---|---|---|---|
| Weibull v1 | 18 | 3 | 6 → 5 | 2 → 2 | 3,7 % → 1,6 % |
| Weibull v2 | 24 | 8 | 8 → 8 | 3 → 2 | 1,6 % → 1,6 % |
| Témoin tirage | 12 | 1 | 7 → 7 | 3 → 2 | 19,3 % → 19,3 % |
| MiniMax v1 tirage | 12 | 6 | 9 → 9 | 2 → 3 | 21,9 % → 22,8 % |
| MiniMax v1 évolution | 12 | 7 | 9 → 7 | 5 → 3 | 23,0 % → 16,5 % |
| MiniMax v2 tirage | 12 | 2 | 5 → 6 | 2 → 3 | 9,8 % → 10,0 % |
| MiniMax v2 évolution | 12 | 5 | 8 → 7 | 4 → 5 | 21,5 % → 18,3 % |

Deux champions deviennent invalides sur la validation (Weibull v1 seed 1017 groupe C,
MiniMax v2 évolution seed 5136). Les plus forts écarts : MiniMax v1 évolution seed 4068
(58,6 % → 27,1 %) et seed 4102 (30,7 % → −5,2 %), MiniMax v2 tirage seed 5102
(8,5 % → 46,6 %), Weibull v2 seed 2170 groupe D (3,0 % → −789 %).

## Ce que cela change

1. **Les verdicts des protocoles ne changent pas.** Tous concluaient à un échec ou à une
   absence de différence montrée, et tous les bras passaient par le même interpréteur.
2. **Les bacs mesurés étaient de vrais rangements**, vérifiés par l'oracle. Mais pour un
   champion sur trois, le programme mesuré n'était pas le programme Rust écrit.
3. **L'indice en faveur de l'évolution est affaibli.** Au seuil de 30 %, l'écart passe de
   5 contre 2 à 3 contre 3 (MiniMax v1) et de 4 contre 2 à 5 contre 3 (MiniMax v2). Au seuil
   de 10 %, de 9 contre 9 à 7 contre 9, et de 8 contre 5 à 7 contre 6.
4. **Ces chiffres « après » ne sont pas des résultats corrigés.** Chaque champion a été
   choisi parce qu'il était le meilleur avec l'ancien interpréteur. Avec le nouveau, un
   autre candidat du même run aurait pu gagner, et l'évolution aurait suivi d'autres
   lignées. Seuls de nouveaux runs avec l'interpréteur corrigé donneront des résultats
   comparables.

Les notes de résultats antérieures restent telles qu'elles ont été écrites. Elles
décrivent des runs faits avec l'ancien interpréteur.
