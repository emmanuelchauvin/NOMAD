# CONTEXTE ET RÔLE
Tu es un architecte logiciel expert en Rust et un chercheur en IA spécialisé dans les algorithmes évolutionnaires dirigés par LLM (État de l'art 2026). Ton objectif est d'implémenter la prochaine phase du projet NOMAD (DARWIN.RS), un moteur évolutionnaire décentralisé en Rust.

Le système a déjà prouvé empiriquement (25 000 instances, 0 crash) que le "Swarm Crossover" surpasse la mutation pure sur le Bin Packing 1D. Cependant, la population a convergé vers un optimum local paramétrique (le plafond de la formule Best-Fit à 1265 bacs), incapable de générer spontanément une branche logique conditionnelle pour gérer les espaces résiduels (les "trous" < 10).

# OBJECTIF : SPÉCIFICATIONS DE NOMAD V2
Tu dois mettre à jour l'architecture Rust (`nomad-orchestrator`, `nomad-genome`, `nomad-evaluator`) pour implémenter les 4 piliers SOTA suivants, visant à briser le plafond de verre de l'optimum local :

## 1. Quality-Diversity (MAP-Elites / QDEvo) dans `nomad-orchestrator`
- **Suppression :** Retirer la sélection compétitive linéaire du "Top 20 %".
- **Implémentation :** Créer une archive `MapElitesArchive`. 
- **Mécanique :** Chaque `SkillCapsule` évaluée voit son code vectorisé (via un embedding léger ou un hash sémantique de son AST). Les capsules sont rangées dans des "niches" de similarité cosinus (seuil $\delta = 0.95$). La compétition (basée sur le score) ne s'applique qu'au sein d'une même niche, garantissant la survie globale d'heuristiques très différentes.

## 2. Opérateur AST à Deux Étapes dans `nomad-genome`
- **Phase 1 (Destruction Déterministe) :** Utiliser la crate `syn` pour parser le code source Rust des deux parents en Arbre Syntaxique Abstrait (AST). Implémenter une fonction déterministe qui hybride brutalement les sous-arbres (ex: injecter la branche `if` du Parent B dans la boucle du Parent A).
- **Phase 2 (Réparation LLM) :** Le code résultant (invalide) est envoyé au LLM via `nomad-operator` avec le prompt de réparation strict : *"Voici une heuristique mutée. Elle contient des erreurs de syntaxe ou de logique. Répare-la pour qu'elle compile et soit fonctionnelle, sans supprimer sa nouvelle structure."*

## 3. Ciblage AgentPSO pour le Swarm Crossover
- Mettre à jour la logique de sélection des parents. Un parent cible n'est plus choisi au hasard.
- L'agent LLM reçoit un prompt de ciblage contenant : 
  1. $P_{best}$ : La meilleure capsule de sa propre lignée.
  2. $G_{best}$ : La meilleure capsule globale de l'archive MAP-Elites.
  3. L'instruction de générer une "vélocité sémantique" : justifier textuellement quelle logique de $G_{best}$ doit être greffée sur $P_{best}$ (ex: *"Je dois intégrer la gestion des objets < 10 issue de G_best"*).

## 4. Évaluation Dynamique ($tLDR$) dans `nomad-evaluator`
- Ne plus évaluer uniquement le nombre final de bacs.
- Enregistrer la trajectoire de l'optimisation (le score après l'objet 10, 20, 30... jusqu'à $N$).
- Calculer le taux de décroissance $tLDR$ (Trajectory-aware Lyapunov Decay Rate) pour récompenser les algorithmes qui convergent vite et restent stables.

---

# PROTOCOLE DE TEST ET VALIDATION (CRITÈRES D'ACCEPTATION)

Pour valider cette nouvelle architecture, tu devras implémenter et réussir les 3 tests scientifiques suivants :

### Test 1 : Le Test de Rupture (Breaking the 1265 Ceiling)
- **Objectif :** Prouver que l'Opérateur AST à deux étapes permet un saut conceptuel.
- **Conditions :** Lancer un run avec le même budget (10 000 appels LLM).
- **Succès :** Au moins une niche MAP-Elites découvre une heuristique générant moins de 1265 bacs sur le dataset In-Distribution, en introduisant explicitement une logique de "dead-space penalty" (gestion conditionnelle des restes < 10) absente de la génération 0.

### Test 2 : Le Test de Résilience Génétique (Quality-Diversity)
- **Objectif :** Prouver que l'effondrement de la diversité (mode collapse) est résolu.
- **Conditions :** Analyser le graphe phylogénétique à la génération 15.
- **Succès :** La population active ne se concentre pas sur une seule lignée. L'archive MAP-Elites maintient au moins 3 "niches" comportementales actives dont la similarité de code source entre elles est inférieure à 60%, tout en ayant des performances acceptables (top 10%).

### Test 3 : Le Test de Généralisation tLDR
- **Objectif :** Prouver que la métrique $tLDR$ prédit mieux la robustesse que le score final.
- **Conditions :** Prendre la capsule gagnante selon la métrique "Bacs classiques" et la capsule gagnante selon la métrique "$tLDR$". Les tester sur le dataset OOD-2 (Loi de puissance).
- **Succès :** La capsule sélectionnée par le $tLDR$ subit une dégradation de performance significativement moindre sur le dataset OOD-2 que celle sélectionnée uniquement par le résultat brut.

Je te propose ces axes d'approfondissement parce qu'en Rust, la distance entre une excellente intuition théorique SOTA et un code qui compile sans faire paniquer le compilateur est immense.Voici pourquoi ces trois briques nécessitent une ingénierie de précision :L'abstraction de la crate syn : Dire au LLM "répare cet AST muté" est la partie facile. Mais coder, côté nomad-genome, le script déterministe qui va parser le code Rust, extraire des syn::ItemFn ou syn::Block, les échanger aléatoirement, puis regénérer du code textuel via la crate quote est un travail d'orfèvre. Si ton module de destruction crashe à cause d'un type inattendu, ta boucle évolutionnaire s'arrête.Le moteur d'embedding pour MAP-Elites : Maintenir une archive Qualité-Diversité exige de ranger les capsules par similarité. Vas-tu embarquer un micro-modèle d'embedding local (via candle ou tch-rs) pour vectoriser le code à la volée, ou utiliser un hachage topologique de l'AST ? Ce choix architectural impactera radicalement la vitesse de ta boucle (budget temps) et la gestion de la mémoire.La discrétisation du $tLDR$ : Transformer l'équation intégrale du taux de décroissance de Lyapunov en code Rust performant au sein de nomad-evaluator demande d'enregistrer les deltas de performance à chaque insertion d'objet dans le Bin Packing. Il faut le coder de manière à ne pas exploser la consommation mémoire lors des 100 000 exécutions du sandbox.

Le choix le plus stratégique et le plus urgent est l'Opérateur de Croisement AST (Phase 1 : Destruction Déterministe) avec les crates syn et quote.Pourquoi ? Parce que c'est le cœur nucléaire de ta rupture scientifique. C'est l'unique mécanisme qui forcera ton LLM à sortir de son optimum local (le plafond des 1265 bacs) en l'obligeant à réparer une structure algorithmique qu'il n'aurait jamais osé générer de lui-même (comme l'ajout brutal d'une branche conditionnelle pour les espaces $< 10$). Les deux autres (MAP-Elites et $tLDR$) sont de l'orchestration ; l'AST, c'est la génétique pure.