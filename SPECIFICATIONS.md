# Spécifications Architecturales et Scientifiques : Projet NOMAD (DARWIN.RS)

## 1. Vision, Hypothèse et Positionnement (SOTA)

NOMAD (DARWIN.RS) est un environnement de recherche et un réseau d'agents autonomes (les "symbiotes") visant à prouver qu'une population de modèles évoluant de manière décentralisée peut générer une intelligence cumulative supérieure à une simple sélection évolutionnaire isolée.

### Positionnement Scientifique
L'état de l'art actuel démontre qu'un LLM couplé à un algorithme évolutionnaire et un évaluateur automatique peut découvrir des heuristiques inédites. La véritable originalité de NOMAD est de démontrer que :
1. Des lignées indépendantes découvrent des heuristiques algorithmiques distinctes.
2. Leur **croisement horizontal (Swarm Crossover)** produit des solutions nouvelles, statistiquement meilleures que celles obtenues par mutation seule.
3. Ces compétences acquises se transmettent, se cumulent et généralisent sur des problèmes hors distribution (Out-Of-Distribution / OOD).

---

## 2. Architecture "Rust-First" (NOMAD CORE)

L'intégralité du moteur d'orchestration, d'évolution et d'évaluation est codée en Rust via un workspace modulaire strictement découplé :

```
nomad/
├── Cargo.toml                  # Cargo Workspace configuration
├── SPECIFICATIONS.md           # Spécifications de référence
├── crates/
│   ├── nomad-core/             # Structures fondamentales, SkillCapsule, traits communs, types de métriques
│   ├── nomad-genome/           # Moteur phylogénétique (DAG via petgraph), traçabilité des mutations & croisements
│   ├── nomad-sandbox/          # Moteur d'exécution sécurisé et déterministe avec comptage de fuel/gas
│   ├── nomad-evaluator/        # Oracle indépendant (jeux de tests Bin Packing 1D: In-Distribution, OOD-1, OOD-2)
│   ├── nomad-operator/         # Abstraction & connecteurs LLM (reproductibilité stricte: seeds, hash GGUF, sampling)
│   ├── nomad-orchestrator/     # Moteur d'évolution, gestion des populations, cycle de vie, budget et Swarm Crossover
│   └── nomad-cli/              # Interface en ligne de commande pour piloter l'arène de recherche
└── scripts/
    └── analysis/               # Scripts d'analyse statistique pré-enregistrée (Wilcoxon, taille d'effet)
```

### Rôles des Modules

* **`nomad-core` :** Définition de la `SkillCapsule`, des identifiants cryptographiques, des résultats d'évaluation, et des configurations d'expérience.
* **`nomad-genome` (Moteur Phylogénétique) :** Modélise l'historique complet des mutations et recombinaisons sous forme de graphe acyclique dirigé (DAG) via `petgraph`, garantissant une traçabilité totale des lignées.
* **`nomad-sandbox` (Moteur d'Exécution) :** Encapsule l'exécution de code sous contrainte déterministe stricte (comptage d'instructions / fuel limit, isolation mémoire, timeout).
* **`nomad-evaluator` (L'Oracle Indépendant) :** Totalement agnostique et inaccessible aux agents lors de la génération. Contient les jeux de données d'évaluation (entraînement, OOD-1, OOD-2), calcule la performance (bacs utilisés) et applique le tie-breaker (gas).
* **`nomad-operator` (Inférence & Traçabilité) :** Interface avec le modèle génératif (local llama.cpp / API / Mock déterministe pour tests). Enregistre systématiquement :
  - Le SHA-256 du modèle
  - La version du moteur d'inférence
  - Le prompt complet et les hyperparamètres de sampling (température, top_p, etc.)
  - La graine stochastique (seed)
* **`nomad-orchestrator` :** Orchestration de la boucle évolutionnaire : instanciation des populations, sélection (ex. top 20%), mutations, croisement sémantique (Swarm Crossover), et comptabilisation des budgets stricts (appels LLM, exécutions).

---

## 3. L'Unité Fondamentale : La Capsule Cognitive (`SkillCapsule`)

La capsule ne contient aucun critère d'évaluation pour interdire tout sur-apprentissage ou exploitation directe de la fonction objectif par l'agent. Une `SkillCapsule` (immuable) contient exclusivement :

1. **L'Interface Sémantique :** Description formelle de la compétence, du problème résolu et de l'intention algorithmique.
2. **Le Moteur Logique :** Le code généré (ex. script déterministe) exécutable dans le sandbox.
3. **Les Métadonnées Phylogénétiques :**
   - Identifiant unique (`CapsuleId`, UUID v4 / hash)
   - Identifiants des parents (1 pour mutation, 2+ pour croisement horizontal)
   - Hash cryptographique SHA-256 du contenu
   - Génération et métadonnées d'exécution

---

## 4. Protocole Expérimental Pré-enregistré

### A. Objectif
Déterminer si le **croisement sémantique (Swarm Crossover)** entre de multiples agents LLM autonomes génère des heuristiques algorithmiques statistiquement supérieures et plus généralisables qu'une sélection évolutionnaire standard à budget de calcul équivalent.

### B. Hypothèses
* **H1 (Performance In-Distribution) :** La médiane des performances de la meilleure heuristique du **Groupe C (Essaim avec croisement)** sera significativement supérieure à celle du **Groupe D (Sélection sans croisement)** sur les instances d'entraînement.
* **H2 (Généralisation OOD) :** L'heuristique gagnante du Groupe C conservera une médiane de performance significativement supérieure à celle du Groupe D sur les jeux Hors-Distribution (**OOD-1** et **OOD-2**).

### C. Problème Cible : Bin Packing Online 1D
- Les objets arrivent séquentiellement un par un.
- L'algorithme place l'objet dans un bac ouvert ou ouvre un nouveau bac, sans pouvoir modifier les placements passés.
- Capacité standard par bac : $C = 100$ (normalisée).

### D. Groupes Expérimentaux (Budget strict : 10 000 appels LLM, 100 000 exécutions par run)
* **Groupe A (Baseline Zéro-Shot / Auto-Correction) :** 1 agent avec 10 000 itérations d'auto-correction séquentielle.
* **Groupe B (Parallèle / Recherche Aléatoire) :** 100 agents effectuant 100 itérations indépendantes (sans transmission).
* **Groupe D (Sélection évolutionnaire standard - Mutation seule) :** Population de 100 individus, sélection du top 20% à chaque génération, reproduction par mutation uniquement (100 générations).
* **Groupe C (Essaim NOMAD - Mutation + Swarm Crossover) :** Population de 100 individus, sélection du top 20% à chaque génération, reproduction combinant mutation et croisement horizontal sémantique entre lignées distinctes (100 générations).

### E. Métriques Hiérarchisées
1. **Métrique Principale :** Nombre de bacs utilisés (à minimiser) comparé à la borne inférieure théorique $B^* = \lceil \frac{\sum w_i}{C} \rceil$.
2. **Métrique Secondaire (Tie-Breaker) :** Consommation de fuel/gas d'exécution.
3. **Pénalité fatale :** Tout code invalide, non déterministe ou dépassant le quota de gas/mémoire se voit attribuer un score de validité nul.

### F. Rigueur Statistique
* 30 runs complets appariés (30 seeds stochastiques distinctes).
* Test statistique de **Wilcoxon apparié** (unilatéral, $\alpha = 0.05$).
* Calcul de la taille d'effet ($r = \frac{Z}{\sqrt{N}}$ / $d$ de Cohen).

### G. Datasets & Empreintes Cryptographiques Pré-enregistrées
* **Entraînement (In-Distribution) :** 50 instances, $N=50$ objets par instance, distribution uniforme $U(10, 80)$.
  * **Hash SHA-256 (Seed 42) :** `04966f900c382331a423fe5d9c0ceea9de8538bd5b00979939e9c3a121f9b504`
* **OOD-1 (Mise à l'échelle) :** 50 instances, $N=500$ objets par instance, distribution uniforme $U(10, 80)$.
  * **Hash SHA-256 (Seed 42) :** `6ec5507854441496733989883f5f555be65cc4ac6c1ad823de0c279ccaab51cf`
* **OOD-2 (Changement de distribution) :** 50 instances, $N=500$ objets par instance, distribution asymétrique (loi de puissance).
  * **Hash SHA-256 (Seed 42) :** `511a4b597decc9a3817491ca099f499ad9fa888abf246c1cdae4a89544844564`

---

## 5. Roadmap de Développement

- **Phase 1 : Le Moteur Évolutionnaire Local (MVP Scientifique)** *(Focus actuel)*
  - Workspace Cargo et crates modulaires.
  - Moteur phylogénétique DAG (`petgraph`).
  - Oracle d'évaluation pour Bin Packing 1D (In-Distribution, OOD-1, OOD-2).
  - Sandbox déterministe avec limite de fuel.
  - Orchestrateur avec boucle évolutionnaire, sélection top 20%, mutations et Swarm Crossover.
  - Outil CLI pour exécuter les benchmarks et exporter les traces JSONL pour analyse statistique.
- **Phase 2 : Le Symbiote Personnel (Produit Standalone)**
  - Interface Tauri, persistance SQLite locale, exécution en tâche de fond.
- **Phase 3 : La Grille Cognitive (Collaboration P2P)**
  - Échange décentralisé de capsules via `libp2p` / DHT.
