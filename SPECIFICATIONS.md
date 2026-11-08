# Spécifications Architecturales et Scientifiques : Projet NOMAD V2 (DARWIN.RS)
## Référence : Architecture Consolidée V2 (Novembre 2026)

---

## 1. Vision, Hypothèse Centrale et Positionnement Scientifique

NOMAD (DARWIN.RS) est un environnement de recherche et un écosystème d'organismes artificiels visant à démontrer expérimentalement l'**accumulation culturelle cumulative**.

### Positionnement Scientifique
L'état de l'art actuel montre que le couplage d'un modèle de langage (LLM) avec des algorithmes génétiques permet de trouver des heuristiques isolées. La véritable rupture scientifique de NOMAD V2 réside dans la démonstration que :
1. **Divergence Comportementale Soutenue (Quality-Diversity) :** L'optimisation ne converge pas vers un attracteur unique (`FIRST_FIT` / plafond empirique de 1265 bacs), mais maintient activement des familles d'heuristiques aux dynamiques comportementales variées via une grille 2D de niches MAP-Elites.
2. **Recombinaison Génétique Structurelle (AST Crossover) :** Les compétences algorithmiques sont manipulées formellement sous forme d'arbres syntaxiques (`syn`/`quote`). Les fragments logiques greffés font l'objet d'un suivi externe strict (`GraftManifest`) et d'une validation causale à 3 niveaux interdisant le code mort.
3. **Orientation Sémantique Ciblée (AgentPSO) :** L'appariement des parents ne repose pas sur le hasard, mais cible la complémentarité comportementale pour compenser les faiblesses diagnostiquées de l'hôte ($Score(B \mid A)$) avec une mémoire de lignée persistante (`SemanticVelocity`).
4. **Signature Dynamique de Compaction ($tLDR$) :** L'évaluation ne se limite pas au score scalaire final, mais modélise la dynamique d'insertion séquentielle par un taux de décroissance de potentiel expérimental ($tLDR$) prédictif de la robustesse hors-distribution (OOD).
5. **Accumulation Culturelle Cumulative (Protocole "Mort et Renaissance") :** Des organismes successifs progressent plus vite et avec un budget d'évaluations inférieur en héritant d'un patrimoine collectif validé (`nomad-archive`), et cet avantage ($CIF > 1.0$) subsiste après destruction intégrale des mémoires de travail et des états individuels.

---

## 2. Architecture Modulaire "Rust-First"

Le workspace Cargo est strictement découplé pour garantir l'étanchéité entre la génération, l'évaluation et l'orchestration :

```
nomad/
├── Cargo.toml                  # Cargo Workspace configuration (syn, quote, rand, chacha, etc.)
├── SPECIFICATIONS.md           # Spécifications de référence consolidées V2
├── Spec V2 bis.md              # Document source exhaustif des spécifications V2
├── crates/
│   ├── nomad-core/             # Structures de données fondamentales, SkillCapsule, traits, métriques
│   ├── nomad-genome/           # Moteur génétique AST (syn/quote), extracteur d'unités, crossover, Niv 1
│   ├── nomad-sandbox/          # Interpréteur AST déterministe avec comptage de fuel, isolation
│   ├── nomad-evaluator/        # Oracle souverain (In-Dist, OOD-1, OOD-2), tLDR, validation Niv 2 & 3
│   ├── nomad-operator/         # Abstraction LLM (connecteur Ollama/llama.cpp, Mock déterministe)
│   ├── nomad-orchestrator/     # Grille MAP-Elites, AgentPSO, moteur d'évolution, protocole Rebirth
│   └── nomad-cli/              # Interface en ligne de commande (run, archive, graph, rebirth, compare)
└── scripts/
    └── analysis/               # Scripts d'analyse statistique et de vérification expérimentale
```

### Rôles des Modules V2
* **`nomad-core` :** Modèle de données immuable (`SkillCapsule`), types canoniques (`ScorePolicy`, `ScoreBinInputs`), empreinte structurelle (`StructuralFingerprint`), manifeste de greffe (`GraftManifest`), descripteur comportemental (`BehaviorDescriptor`), vélocité sémantique (`SemanticVelocity`), et métriques scientifiques cumulatives (`CumulativeTransmissionMetrics`).
* **`nomad-genome` :** Analyseur syntaxique formel de la fonction `score_bin(...) -> f64` via `syn`, extracteur de fragments logiques (`LogicalUnit`), opérateur de croisement déterministe générant le `GraftManifest` et régénérant le code via `quote`, et contrôle de Niveau 1 de survie structurelle.
* **`nomad-sandbox` :** Interpréteur AST déterministe avec comptage strict de fuel logique (`AstInterpreter`), isolation mémoire et simulation séquentielle de placement pour chaque bac candidat.
* **`nomad-evaluator` :** Oracle souverain indépendant contenant les jeux de données sanctuarisés (In-Distribution, OOD-1, OOD-2), calculateur exact du $tLDR$ par régression linéaire, et pipeline complet de validation Anti-Dead-Code à 3 niveaux (avec capsule d'ablation $C_{\text{null}}$).
* **`nomad-operator` :** Connecteurs d'inférence (Ollama, llama.cpp, Mock reproductible) traçant les hashes de modèle, les graines aléatoires, les hyperparamètres et les prompts sous contraintes strictes.
* **`nomad-orchestrator` :** Grille 2D Quality-Diversity MAP-Elites (`MapElitesArchive`) avec compétition locale intra-niche et curiosité UCB, gestionnaire de lignées AgentPSO (`LineageManager`), et moteur du protocole "Mort et Renaissance" (`execute_rebirth_protocol`).
* **`nomad-cli` :** Outil en ligne de commande unifié permettant d'exécuter les simulations (`run`), d'inspecter les niches (`archive`), de visualiser les arbres de filiation en HTML/Mermaid (`graph`), et de mesurer la transmission culturelle (`rebirth`).

---

## 3. Les Cinq Piliers Fondamentaux de NOMAD V2

### Pilier 1 — Quality-Diversity & MAP-Elites
Remplacement de la sélection linéaire globale (Top 20%) par une grille discrétisée de niches comportementales $10 \times 10 = 100$ cellules :
* **Axe X (Perte Sèche) :** Ratio d'espace mort (`dead_space_ratio`, résidus $< 10$).
* **Axe Y (Expansion) :** Ratio d'ouverture de nouveaux bacs (`new_bin_ratio`).
* **Compétition locale :** Un nouvel organisme ne combat que l'occupant de sa propre cellule.
* **Sélection par Curiosité UCB :**
  $$UCB(\text{niche}) = \text{Qualité}(\text{niche}) + c \cdot \sqrt{\frac{\ln(N_{\text{total}})}{N_{\text{niche}}}}$$

### Pilier 2 — Crossover Génétique AST & Validation Anti-Dead-Code
* **Interface canonique `ScorePolicy` :**
  ```rust
  pub fn score_bin(
      item_size: u32,
      bin_residual: u32,
      bin_capacity: u32,
      step_index: usize,
      total_items: usize,
      open_bins_count: usize,
      mean_residual: f64,
  ) -> f64
  ```
* **Manifeste de Greffe Externe (`GraftManifest`) :** Traçabilité indépendante du code (identifiant de greffe, hashes des parents, hash du fragment greffé, locus d'insertion, hash des dépendances).
* **Validation Anti-Dead-Code à 3 Niveaux :**
  1. *Niveau 1 (Structurel) :* Présence et intégrité vérifiée du fragment dans l'AST enfant.
  2. *Niveau 2 (Computationnel) :* Activation effective en simulation avec consommation mesurée de fuel.
  3. *Niveau 3 (Comportemental) :* Création de la capsule d'ablation témoin $C_{\text{null}}$ et vérification différentielle de la causalité comportementale sur un jeu de sondes.

### Pilier 3 — AgentPSO et Vélocité Sémantique
* **Score de Sélection du Partenaire :**
  $$Score(B \mid A) = 0.50 \cdot Q(B) + 0.35 \cdot C_{\text{comp}}(A, B) + 0.15 \cdot D_{\text{ast}}(A, B)$$
* **Vecteur `SemanticVelocity` :** Identification précise de la faiblesse de l'hôte, du fragment donneur, et de l'action de momentum (`ContinueDirection`, `Pivot`, `Combine`).
* **Mémoire de Lignée :** Conservation de $P_{\text{best}}$ (meilleur ancêtre) et $G_{\text{best\_partner}}$.

### Pilier 4 — Évaluation Dynamique & $tLDR$
* **Borne Volumétrique Préfixe :**
  $$L_{\text{vol}}(t) = \left\lceil \frac{\sum_{i=1}^{t} w_i}{C} \right\rceil$$
* **Écart Relatif Expérimental (Waste) :**
  $$V(t) = \frac{B(t) - L_{\text{vol}}(t)}{L_{\text{vol}}(t)}$$
* **Régression Linéaire :** Sur la fenêtre $t \in [\lfloor N/5 \rfloor, N]$, ajustement par moindres carrés de $y(t) = \ln(V(t) + 10^{-4})$ sur le temps normalisé $\tau = t / N$ pour extraire la pente $\beta_1$ et l'écart-type résiduel $\sigma_{\text{res}}$ :
  $$tLDR = -\beta_1 - 0.5 \cdot \sigma_{\text{res}}$$
* **Sanctuarisation OOD :** Les benchmarks OOD-1 et OOD-2 ne participent jamais à la sélection des parents.

### Pilier Transversal — Accumulation Cumulative & Protocole "Mort et Renaissance"
Protocole scientifique en double-aveugle validant l'hypothèse centrale (Section 7.3 & 8.5) :
1. **Époque 1 (Genèse) :** Évolution d'une population $P_1$ et constitution du patrimoine collectif validé (`nomad-archive`).
2. **L'Extinction :** Destruction totale des mémoires de travail, des contextes d'agents et des états PSO.
3. **Époque 2 (Héritage vs Tabula Rasa) :** Confrontation entre la Population A (accès en lecture au patrimoine) et la Population B (départ de zéro), sur **la même seed et le même budget d'évaluations**.
4. **Métriques Scientifiques :**
   * **Cumulative Improvement Factor :** $CIF = \frac{\text{Excès Tabula Rasa}}{\text{Excès Héritière}} > 1.0$
   * **Functional Heritability Rate :** $R_{\text{herit}} = \frac{\text{Motifs transmis actifs}}{\text{Motifs initiaux}}$
   * **Innovation Rate :** $R_{\text{innov}} = \frac{\text{Innovations validées}}{\text{Évaluations}} \times 1000$

---

## 4. Modèle de Données : La Capsule Cognitive V2 (`SkillCapsule`)

Une capsule est immuable et exempte de métriques de score d'évaluation :

```rust
pub struct SkillCapsule {
    pub id: CapsuleId,
    pub name: String,
    pub semantic_interface: String,
    pub logic_code: String,
    pub parent_ids: Vec<CapsuleId>,
    pub generation: usize,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    pub interface_version: String,
    pub policy_kind: PolicyKind,
    pub ast_fingerprint: Option<StructuralFingerprint>,
    pub dependencies: Vec<String>,
    pub validation: ValidationStatus,
    pub graft_manifest: Option<GraftManifest>,
}
```

---

## 5. Interface CLI Unifiée (`nomad-cli`)

* **`nomad run` :** Lance une simulation évolutionnaire. Sauvegarde automatiquement dans `run_report.json` par défaut.
  ```powershell
  cargo run --release --bin nomad -- run --operator ollama --model qwen2.5-coder:7b --pop-size 8 -n 10 --group c
  ```
* **`nomad archive` :** Inspecte immédiatement les cellules occupées de la grille MAP-Elites et le front de Pareto comportemental.
  ```powershell
  cargo run --release --bin nomad -- archive
  ```
* **`nomad graph` :** Exporte l'arbre généalogique complet en HTML interactif avec rendu Mermaid.
  ```powershell
  cargo run --release --bin nomad -- graph --format html --output arbre.html
  ```
* **`nomad rebirth` :** Exécute le protocole scientifique complet "Mort et Renaissance" et compare les populations Héritière et Tabula Rasa.
  ```powershell
  cargo run --release --bin nomad -- rebirth --operator ollama --model qwen2.5-coder:7b --pop-size 6 -n 4 --output rebirth.json
  ```
* **`nomad baseline` :** Exécute les heuristiques classiques (First-Fit, Best-Fit, Worst-Fit) sur les benchmarks officiels.
* **`nomad dataset` :** Vérifie les empreintes cryptographiques SHA-256 des jeux de données d'évaluation.

---

## 6. Datasets Pré-enregistrés et Sanctuarisés (Seed 42)

* **Entraînement (In-Distribution) :** 50 instances, $N=50$ objets, distribution uniforme $U(10, 80)$, $C=100$.
  * SHA-256 : `04966f900c382331a423fe5d9c0ceea9de8538bd5b00979939e9c3a121f9b504`
* **OOD-1 (Mise à l'échelle) :** 50 instances, $N=500$ objets, distribution uniforme $U(10, 80)$, $C=100$.
  * SHA-256 : `6ec5507854441496733989883f5f555be65cc4ac6c1ad823de0c279ccaab51cf`
* **OOD-2 (Décalage de distribution) :** 50 instances, $N=500$ objets, distribution asymétrique (loi de puissance), $C=100$.
  * SHA-256 : `511a4b597decc9a3817491ca099f499ad9fa888abf246c1cdae4a89544844564`

---

## 7. Matrice d'Expérimentation et d'Ablation Scientifique

| Groupe | MAP-Elites | AST Crossover | AgentPSO | $tLDR$ | Objectif Scientifique |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **A — Baseline V1** | Non | Non | Non | Non | Référence historique (Top 20% linéaire) |
| **B — QD Only** | Oui | Non | Non | Non | Impact isolé de la diversité comportementale |
| **C — AST Only** | Non | Oui | Non | Non | Impact du croisement syntaxique formel |
| **D — QD + AST** | Oui | Oui | Non | Non | Synergie diversité et recombinaison |
| **E — QD + AST + PSO** | Oui | Oui | Oui | Non | Valeur ajoutée du routage complémentaire |
| **F — V2 Full** | Oui | Oui | Oui | Oui | Évaluation globale du système complet |
