# Spécifications Architecturales et Scientifiques : Projet NOMAD (DARWIN.RS)

## 1. Vision, Hypothèse et Positionnement (SOTA)

NOMAD (DARWIN.RS) est un environnement de recherche et un réseau d'agents autonomes (les "symbiotes") visant à prouver qu'une population de modèles évoluant de manière décentralisée peut générer une intelligence cumulative supérieure à une simple sélection évolutionnaire isolée. 

**Positionnement Scientifique :** L'état de l'art actuel démontre qu'un LLM couplé à un algorithme évolutionnaire et un évaluateur automatique peut découvrir des heuristiques inédites. La véritable originalité de NOMAD est de démontrer que :
1. Des lignées indépendantes découvrent des heuristiques différentes.
2. Leur **croisement horizontal (Swarm Crossover)** produit des solutions nouvelles, meilleures que celles obtenues par mutation seule.
3. Ces compétences acquises se transmettent, se cumulent et généralisent sur des problèmes hors distribution.

## 2. Architecture "Rust-First" (NOMAD CORE)

L'intégralité du moteur d'orchestration, d'évolution et d'évaluation est codée en Rust. L'architecture est découpée en modules strictement découplés :

* **`nomad_orchestrator` :** Gère le cycle de vie, alloue les budgets de calcul, gère les populations et cadence les générations.
* **`nomad_operator` (Inférence Locale) :** Interface avec le modèle (via `llama.cpp`). **Garantie de reproductibilité :** Enregistre le *hash* SHA-256 du fichier GGUF exact, la version de llama.cpp, le prompt complet, les paramètres de sampling (température, top_p) et la *seed* stochastique.
* **`nomad_sandbox` (Moteur d'Exécution) :** Encapsule `wasmtime`. Pour le MVP, le sandbox instancie un unique interpréteur pré-compilé en Wasm (**QuickJS**) afin de garantir un comptage équitable et déterministe du coût des opérations (*fuel/gas limit*).
* **`nomad_evaluator` (L'Oracle Indépendant) :** Totalement indépendant des agents. Contient les jeux de tests (visibles et cachés) et calcule la performance de l'algorithme.
* **`nomad_genome` (Moteur Phylogénétique) :** Modélise l'historique des mutations (DAG) via `petgraph`, garantissant la traçabilité.

## 3. L'Unité Fondamentale : La Capsule Cognitive (`SkillCapsule`)

La capsule ne contient aucun critère d'évaluation pour éviter l'exploitation des métriques par l'agent. Une `SkillCapsule` (immuable) contient exclusivement :

1. **L'Interface Sémantique :** Description de la compétence et intention de l'algorithme.
2. **Le Moteur Logique :** Le code généré (script JavaScript) exécutable dans le sandbox QuickJS/Wasm.
3. **Les Métadonnées :** Version, ID(s) parent(s), hash cryptographique, et dépendances.

## 4. Roadmap de Développement par Étapes

### Phase 1 : Le Moteur Évolutionnaire Local (MVP Scientifique)
* **Objectif :** Construire l'arène de test locale et valider statistiquement le croisement.
* **Livrables :** `nomad_orchestrator`, `nomad_operator`, bac à sable Wasm (QuickJS), Moteur DAG (`nomad_genome`), oracle (`nomad_evaluator`), et scripts d'analyse statistique.

### Phase 2 : Le Symbiote Personnel (Produit Standalone)
* **Objectif :** Transformer le moteur expérimental en assistant persistant (Tauri, SQLite).

### Phase 3 : La Grille Cognitive (Collaboration P2P)
* **Objectif :** Connecter les symbiotes (libp2p, DHT) pour distribuer l'évolution à l'échelle d'Internet.

---

# ANNEXE : Document de Pré-enregistrement Expérimental

## A. Objectif de l'Étude
L'objectif de cette expérience est de déterminer si le mécanisme de **croisement sémantique (Swarm Crossover)** entre de multiples agents LLM autonomes permet de générer des heuristiques algorithmiques statistiquement supérieures et plus généralisables qu'une sélection évolutionnaire standard à budget de calcul équivalent.

## B. Hypothèses
*   **H1 (Performance In-Distribution) :** La médiane des performances de la meilleure heuristique générée par le Groupe C (Essaim avec croisements) sera significativement supérieure à celle du Groupe D (Sélection sans croisements) sur les instances d'entraînement.
*   **H2 (Généralisation OOD) :** L'heuristique gagnante du Groupe C conservera une médiane de performance significativement supérieure à l'heuristique gagnante du Groupe D lorsqu'évaluée sur les jeux de données Hors-Distribution (OOD-1 et OOD-2).

## C. Protocole et Variables Indépendantes
Le problème cible est le **Bin Packing Online 1D** (les objets arrivent un par un, l'algorithme ne peut pas réorganiser les objets déjà placés).
Le budget strict par groupe pour une répétition est de **10 000 appels au LLM** et **100 000 exécutions Wasm**.

Les 4 groupes expérimentaux sont :
*   **Groupe A (Baseline Zéro-Shot/Correction) :** 1 agent avec 10 000 itérations d'auto-correction séquentielle.
*   **Groupe B (Parallèle) :** 100 agents effectuant 100 itérations indépendantes (aucune transmission).
*   **Groupe D (Sélection évolutionnaire avec mutation et prompting multi-exemples, sans opérateur de croisement explicite) :** Population de 100 individus. Sélection du top 20% à chaque génération. Reproduction par **mutation uniquement**. (100 générations).
*   **Groupe C (Essaim NOMAD) :** Population de 100 individus. Sélection du top 20% à chaque génération. Reproduction par **mutation et croisement sémantique** entre les lignées. (100 générations).

## D. Métriques (Variables Dépendantes)
L'évaluation par `nomad_evaluator` repose sur deux axes hiérarchisés :
1.  **Métrique Principale :** Le **nombre de bacs utilisés** (à minimiser) comparé à une borne inférieure théorique.
2.  **Métrique Secondaire (Contrainte) :** Le coût d'exécution (gas Wasm). Utilisé comme *tie-breaker* si deux heuristiques utilisent le même nombre de bacs, avec un plafond d'exécution strict.
*Pénalité fatale :* Tout code échouant à l'exécution ou dépassant le plafond de calcul/mémoire est éliminé (score nul).

## E. Rigueur Statistique et Seuils
*   **Taille de l'échantillon :** L'expérience complète sera exécutée **30 fois** (30 *seeds* stochastiques distinctes pour le LLM). Les *seeds* sont appariées entre les groupes.
*   **Test Statistique :** **Test de Wilcoxon apparié** (unilatéral) pour comparer les performances entre les Groupes C et D.
*   **Analyse complémentaire :** Calcul de la taille de l'effet (ex: d de Cohen ou r de Wilcoxon) et intervalle de confiance.
*   **Seuil de significativité :** $\alpha = 0.05$. L'hypothèse est validée si $p < 0.05$ et que la taille de l'effet est non négligeable.

## F. Verrouillage OOD (Out-Of-Distribution)
*   **Instances d'entraînement (In-Distribution) :** 50 instances de Bin Packing Online avec des séquences de $N=50$ objets.
*   **Instances OOD-1 (Mise à l'échelle) :** 50 instances avec $N=500$ objets, même distribution statistique que l'entraînement.
*   **Instances OOD-2 (Changement de distribution) :** 50 instances avec $N=500$ objets, distribution asymétrique (loi de puissance).
*   **Empreinte cryptographique de sécurité :** 
    *   Hash SHA-256 du dataset d'entraînement : `04966f900c382331a423fe5d9c0ceea9de8538bd5b00979939e9c3a121f9b504`
    *   Hash SHA-256 du dataset OOD-1 : `6ec5507854441496733989883f5f555be65cc4ac6c1ad823de0c279ccaab51cf`
    *   Hash SHA-256 du dataset OOD-2 : `511a4b597decc9a3817491ca099f499ad9fa888abf246c1cdae4a89544844564`