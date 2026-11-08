# NOMAD V2 — DARWIN.RS
## Spécification technique et scientifique complète
**Version 2.2 — Édition consolidée intégrant la revue critique V2.1 — Novembre 2026**

---

# 0. STATUT ET CONVENTIONS DE LA SPÉCIFICATION

Cette spécification définit l'architecture cible de NOMAD V2, ses mécanismes évolutionnaires, ses exigences techniques et le protocole permettant d'évaluer scientifiquement ses résultats.

Elle constitue un cadre de développement et d'expérimentation, pas une affirmation que tous les mécanismes proposés sont déjà validés.

Trois catégories doivent être distinguées pendant toute l'implémentation :

- **Exigence technique** : comportement que le logiciel doit garantir.
- **Hypothèse scientifique** : mécanisme dont l'intérêt doit être vérifié expérimentalement.
- **Critère expérimental** : résultat mesurable permettant de soutenir ou de réfuter une hypothèse.

Une fonctionnalité qui compile n'est pas nécessairement une fonctionnalité utile. Une amélioration sur un benchmark n'est pas nécessairement une innovation algorithmique. Une compétence présente dans plusieurs générations n'est pas nécessairement une compétence transmise fonctionnellement.

Ces distinctions sont fondamentales.

# 1. CONTEXTE ET VISION DU PROJET

## 1.1. Rôle de NOMAD

NOMAD (DARWIN.RS) est un moteur évolutionnaire destiné à faire émerger des compétences algorithmiques nouvelles par recombinaison, évaluation, sélection, conservation et transmission entre générations d'organismes artificiels.

Le système doit explorer une hypothèse ambitieuse :

**Une population d'organismes artificiels peut-elle développer progressivement un patrimoine collectif de compétences, dans lequel les découvertes d'une génération deviennent le point de départ des générations suivantes, permettant une progression cumulative qui dépasse les capacités d'un organisme isolé ?**

L'objectif n'est donc pas simplement de construire un optimiseur de Bin Packing.

Il s'agit de concevoir un environnement expérimental dans lequel des organismes artificiels peuvent :

1. Développer individuellement des compétences.
2. Découvrir de nouvelles structures algorithmiques.
3. Recombiner leurs compétences avec celles d'autres organismes.
4. Évaluer objectivement les résultats de ces recombinaisons.
5. Conserver les découvertes réellement utiles.
6. Transmettre ces découvertes à d'autres organismes.
7. Construire progressivement un patrimoine collectif plus performant que les connaissances initiales.

Le projet doit permettre de distinguer une simple optimisation évolutionnaire d'un véritable mécanisme d'accumulation cumulative.

## 1.2. État actuel de NOMAD V1

Les expérimentations V1 ont montré :

- 25 000 instances traitées sans crash constaté pendant les expérimentations rapportées.
- Un mécanisme *Swarm Crossover* qui a surpassé la mutation pure sur le problème du Bin Packing 1D testé.
- Une capacité de recombinaison et de transmission de stratégies entre organismes.

Une limite a cependant été observée : la population converge vers une famille de stratégies paramétriques apparentées à Best-Fit, avec un plafond empirique de **1265 bacs sur le benchmark de référence utilisé**.

Les organismes améliorent progressivement les poids et les paramètres de leurs heuristiques, mais ne semblent pas parvenir à introduire spontanément de nouvelles structures logiques suffisamment efficaces.

Un exemple de structure recherchée serait l'apparition d'une pénalité conditionnelle non linéaire sur certains espaces résiduels, éventuellement par paliers.

Le problème à explorer n'est donc plus seulement l'optimisation des paramètres, mais l'évolution de la structure même des algorithmes.

Le résultat de 1265 bacs est une observation propre au benchmark et aux conditions expérimentales de V1. Il ne doit pas être considéré comme un plafond universel de Best-Fit.

## 1.3. Ambition de NOMAD V2

NOMAD V2 doit introduire quatre piliers évolutionnaires :

1. **Quality-Diversity / MAP-Elites** : préserver durablement des familles de stratégies comportementalement différentes.
2. **Crossover structurel AST** : permettre le transfert de fragments algorithmiques réellement actifs entre organismes.
3. **AgentPSO et vélocité sémantique** : orienter les croisements vers des compétences complémentaires.
4. **Évaluation dynamique et tLDR** : analyser les trajectoires d'exécution et étudier leur éventuel pouvoir prédictif sur la généralisation.

Une cinquième dimension transversale est instrumentée dès cette version :

5. **Accumulation cumulative des compétences** : mesurer la conservation, la transmission et l'amélioration du patrimoine collectif entre générations.

## 1.4. Ce que NOMAD V2 ne cherche pas encore à démontrer

NOMAD V2 ne prétend pas démontrer :

- L'émergence d'une intelligence générale.
- L'apparition spontanée d'une conscience artificielle.
- Une supériorité universelle des systèmes multi-agents.
- Une capacité générale d'innovation indépendante du domaine.
- Une forme d'évolution biologique au sens strict.

La première démonstration recherchée est plus circonscrite et mesurable :

**Une population d'organismes logiciels peut-elle découvrir, recombiner et transmettre des compétences algorithmiques de manière cumulative, avec un avantage mesurable sur une population qui repart de zéro ?**

Le Bin Packing 1D constitue le premier laboratoire expérimental, pas la destination finale.

---

# 2. ARCHITECTURE GÉNÉRALE

## 2.1. Principes architecturaux

L'architecture repose sur un workspace Cargo modulaire en Rust.

Les responsabilités doivent être séparées afin de pouvoir expérimenter indépendamment sur :

- Les mécanismes évolutionnaires.
- La représentation des compétences.
- Les opérateurs génétiques.
- L'évaluation.
- Les capacités de réflexion LLM.
- La conservation du patrimoine.
- La reproductibilité scientifique.

Les organismes ne doivent pas contrôler leur propre évaluation. Le moteur d'évaluation reste indépendant du code et des affirmations produits par les organismes.

## 2.2. Organisation des crates

### `nomad-orchestrator`

**Responsabilité : piloter l'évolution de la population.**

Fonctionnalités :

- Gestion des populations et des générations.
- Cycle de vie des organismes.
- `MapElitesArchive` : sélection et maintien de la diversité.
- Sélection des parents.
- Quality-Diversity selection.
- `AgentPSO` : routage sémantique des partenaires.
- Gestion des lignées.
- Gestion des budgets d'évolution.
- Coordination des opérations de mutation et de crossover.

L'orchestrateur ne doit pas contenir la logique métier des heuristiques ni celle de leur évaluation.

### `nomad-genome`

**Responsabilité : représenter, analyser et recombiner les compétences.**

Fonctionnalités :

- `SkillCapsule` : représentation canonique d'une compétence.
- Parsing Rust avec `syn`.
- Reconstruction avec `quote`.
- Extraction d'unités logiques.
- Crossover AST déterministe.
- Mutation structurelle.
- Gestion des dépendances entre fragments.
- Manifeste de provenance génétique.
- Empreintes AST canoniques.
- Comparaison structurelle des compétences.

### `nomad-evaluator`

**Responsabilité : compiler et évaluer les compétences dans un environnement isolé.**

Fonctionnalités :

- Compilation vers WebAssembly.
- Exécution sandboxée avec Wasmtime.
- Gestion des limites de mémoire et de calcul.
- Calcul de la fitness.
- Enregistrement des trajectoires.
- Calcul expérimental du tLDR.
- Tests différentiels Anti-Dead-Code.
- Évaluation de robustesse.
- Évaluation OOD.
- Calcul des descripteurs comportementaux.

### `nomad-operator`

**Responsabilité : fournir les capacités cognitives et réflexives des organismes.**

Fonctionnalités :

- Interface LLM asynchrone.
- Réflexion sémantique.
- Analyse des échecs.
- Génération de vélocité sémantique.
- Planification des croisements.
- Interprétation des compétences.
- Réparation AST assistée par LLM.
- Explication des résultats expérimentaux.

Le LLM propose et transforme. Il ne décide jamais seul de la validité ou de la qualité d'une compétence.

### `nomad-experiment`

**Responsabilité : garantir la rigueur expérimentale.**

Fonctionnalités :

- Configuration déclarative des expériences.
- Gestion des seeds.
- Gestion des budgets LLM, CPU et évaluations.
- Orchestration des ablations.
- Gestion des datasets ID et OOD.
- Conservation des résultats expérimentaux.
- Calcul des intervalles de confiance.
- Calcul des tailles d'effet.
- Tests statistiques.
- Génération de rapports reproductibles.

### `nomad-archive`

**Responsabilité : conserver le patrimoine génétique et cognitif collectif.**

Fonctionnalités :

- Stockage persistant des `SkillCapsules`.
- Métadonnées comportementales.
- Historique des niches.
- Empreintes AST.
- Provenance des compétences.
- Historique des générations.
- Graphe phylogénétique sous forme de DAG.
- Stockage des résultats de validation.
- Historique des transferts et recombinaisons.
- Gestion des versions du patrimoine.

Le patrimoine collectif doit être indépendant de la mémoire de travail des organismes.

## 2.3. Flux fonctionnel d'une itération évolutionnaire

1. L'orchestrateur sélectionne un organisme hôte dans `MapElitesArchive`.
2. L'organisme dispose de sa compétence actuelle et de son historique individuel.
3. `AgentPSO` analyse les performances, les faiblesses et les directions d'évolution possibles.
4. L'orchestrateur sélectionne un partenaire génétique dans une niche complémentaire.
5. `nomad-genome` identifie les unités logiques transférables et réalise un crossover déterministe.
6. Si nécessaire, `nomad-operator` répare le code hybride sous contraintes.
7. `nomad-evaluator` compile la nouvelle capsule et exécute les contrôles de validité.
8. Le fragment greffé est soumis aux tests de survie fonctionnelle.
9. La capsule est évaluée sur les benchmarks d'entraînement.
10. Les trajectoires et les descripteurs comportementaux sont calculés.
11. La capsule candidate est insérée ou rejetée par l'archive Quality-Diversity.
12. `nomad-archive` conserve la provenance et les résultats.
13. Les informations utiles alimentent les réflexions et les croisements suivants.

Une capsule rejetée peut rester enregistrée dans les journaux expérimentaux, mais elle ne doit pas contaminer le patrimoine des compétences validées.

---

# 3. PILIER 1 — QUALITY-DIVERSITY / MAP-ELITES

## 3.1. Objectif

La sélection globale de type Top 20 % peut provoquer une convergence prématurée vers une famille de stratégies dominantes.

MAP-Elites doit remplacer cette compétition globale par une compétition locale au sein de niches comportementales.

L'objectif est de conserver des stratégies différentes, même si certaines sont initialement moins performantes.

Une stratégie secondaire peut devenir un parent particulièrement intéressant si elle possède un mécanisme que les meilleures stratégies ne possèdent pas.

La diversité n'est donc pas seulement une métrique descriptive. Elle constitue une ressource pour l'évolution future.

## 3.2. Structure `MapElitesArchive`

L'archive contient :

- La topologie des niches.
- Les centroïdes ou les limites de la grille.
- Les capsules élites de chaque niche.
- Les descripteurs comportementaux normalisés.
- Les empreintes structurelles AST.
- Les scores de performance.
- Les métadonnées de provenance.
- L'historique des remplacements.
- Les statistiques d'exploration de chaque niche.

Chaque capsule possède au minimum :

```rust
pub struct ArchiveEntry {
    pub capsule_id: CapsuleId,
    pub niche_id: NicheId,
    pub behavior: BehaviorDescriptor,
    pub structural_fingerprint: StructuralFingerprint,
    pub fitness: FitnessMetrics,
    pub provenance: GeneticProvenance,
    pub validation: ValidationStatus,
}
```

## 3.3. Descripteurs comportementaux

Les descripteurs comportementaux constituent l'espace primaire de nichage.

Les métriques candidates à enregistrer sont :

1. Distribution des espaces résiduels.
2. Entropie ou variance des espaces résiduels non nuls.
3. Fréquence de création de nouveaux bacs.
4. Taux moyen de remplissage.
5. Répartition des décisions selon l'espace résiduel.
6. Comportement sur les petits objets.
7. Sensibilité à l'ordre des objets.
8. Répartition des décisions et espaces inutilisés finaux.

Les descripteurs doivent être calculés sur un ensemble de sondes fixe et distinct des données OOD.

Il faut également vérifier qu'ils distinguent effectivement des familles de comportements utiles. Un descripteur qui varie sans correspondre à une différence algorithmique intéressante n'est pas nécessairement un bon axe de nichage.

## 3.4. Projection des descripteurs

Il est interdit de construire une grille cartésienne directe sur huit dimensions avec cinq niveaux par dimension : cela créerait 390 625 niches, dont une très grande majorité resterait vide.

Deux modes sont supportés.

### A. Grille dense 2D/3D

Mode par défaut.

Sélection de deux ou trois descripteurs suffisamment indépendants.

Exemple :

- Axe 1 : entropie ou variance des espaces résiduels.
- Axe 2 : ratio d'ouverture de nouveaux bacs.
- Axe 3 éventuel : sensibilité à l'ordre.

Configurations initiales possibles :

- Grille 10 × 10 : 100 niches.
- Grille 5 × 5 × 4 : 100 niches.

Les bornes doivent être définies sur un jeu de calibration indépendant et ne pas être modifiées opportunément pour améliorer les résultats.

### B. CVT-MAP-Elites

Mode expérimental pour exploiter davantage de descripteurs.

- Vecteurs comportementaux normalisés.
- Nombre de niches fixe, initialement compris entre 50 et 100.
- Centroïdes calculés par K-Means sur un échantillon de calibration.
- Affectation d'une capsule à son centroïde le plus proche.

Les centroïdes sont gelés pendant une campagne expérimentale, sauf expérience explicitement consacrée à leur adaptation.

## 3.5. Représentation structurelle secondaire

La diversité syntaxique ne doit jamais être confondue avec la diversité comportementale.

`nomad-genome` calcule un fingerprint structurel léger de l'AST :

- Histogramme des profondeurs.
- Nombre de branches `if` et `match`.
- Nombre de transformations arithmétiques.
- Présence de seuils et de pénalités.
- Nombre d'appels auxiliaires.
- Motifs topologiques.
- Hash structurel de type Weisfeiler-Lehman, si sa définition est stabilisée.

Un embedding sémantique externe reste optionnel.

Cette représentation sert à analyser les lignées, identifier des motifs nouveaux et départager certaines égalités. Elle ne doit pas remplacer le comportement mesuré pour déterminer la niche primaire.

## 3.6. Sélection et compétition locale

Une capsule candidate :

1. Passe les contrôles de validité.
2. Est évaluée sur le jeu ID.
3. Reçoit un vecteur comportemental.
4. Est affectée à une niche.
5. Est comparée au représentant de cette niche.

La compétition est locale.

Il ne faut pas réintroduire indirectement une sélection globale Top 20 % qui éliminerait toutes les stratégies sous-optimales mais complémentaires.

### Critère d'insertion

Le système doit distinguer deux cas :

- **Fitness scalaire** : le représentant est remplacé si la candidate améliore le critère de performance défini.
- **Sélection multi-objectifs** : les solutions non dominées sont conservées selon une politique explicitement définie.

Pour la V2, le choix opérationnel recommandé est un représentant principal par niche, accompagné d'un petit ensemble optionnel de solutions Pareto lorsque la valeur expérimentale le justifie.

La règle de remplacement doit être déterministe à seed identique.

En cas d'égalité, la diversité structurelle peut servir de critère secondaire, mais ne doit pas permettre de conserver indéfiniment du code syntaxiquement différent sans valeur comportementale.

## 3.7. Exploration des niches

L'échantillonnage des parents utilise un mécanisme de curiosité de type UCB.

Il peut favoriser :

- Les niches récemment découvertes.
- Les niches ayant produit des descendants viables.
- Les niches peu explorées.
- Les niches dont les compétences semblent complémentaires aux faiblesses du parent courant.

Les paramètres de curiosité doivent être configurables et enregistrés dans les expériences.

## 3.8. Métriques Quality-Diversity

À chaque génération, enregistrer :

- Couverture de l'archive.
- Nombre de niches occupées.
- QD-Score.
- Fitness moyenne et meilleure fitness.
- Diversité comportementale.
- Diversité structurelle.
- Taux de remplacement des représentants.
- Taux de découverte de nouvelles niches.
- Taux de descendants viables par niche.

### Définition du QD-Score

L'orientation de la fitness doit être explicite.

Pour le Bin Packing, moins de bacs signifie une meilleure performance. Le QD-Score doit donc utiliser une transformation de score dont le sens est clair, par exemple :

\[
q(c)= -B(c)
\]

ou une transformation positive normalisée définie avant l'expérience.

Le QD-Score est alors :

\[
QD=\sum_{i \in \text{niches occupées}} q(c_i)
\]

La normalisation éventuelle doit être identique pour tous les groupes comparés.

Il est interdit de changer la définition du QD-Score après observation des résultats.

---

# 4. PILIER 2 — OPÉRATEUR GÉNÉTIQUE AST

## 4.1. Objectif

Le crossover AST doit permettre le transfert de mécanismes algorithmiques entre organismes.

Il ne s'agit pas de mélanger arbitrairement du texte source.

Il s'agit d'identifier des unités logiques, de comprendre leurs dépendances, de les transférer dans un autre contexte, puis de vérifier qu'elles contribuent effectivement au comportement de la nouvelle capsule.

Les bibliothèques principales sont :

- `syn` pour analyser l'AST Rust.
- `quote` pour reconstruire du code Rust.

` syn` ne réalise pas l'analyse sémantique complète du compilateur Rust. Il ne résout pas à lui seul les types, les lifetimes, les traits ou les règles du borrow checker.

Le système doit donc séparer l'analyse structurelle, la réparation et la validation par compilation.

## 4.2. Signature canonique : commencer simple sans enfermer l'évolution

La V2 doit utiliser une interface canonique simple pour son premier domaine expérimental, le Bin Packing 1D.

Cependant, **la signature `score_bin` ne doit pas devenir la définition permanente de toute compétence possible dans NOMAD**.

Une simple fonction de scoring est pratique pour démarrer, mais elle limite l'espace génétique :

- Elle représente mal les stratégies qui ont besoin d'un état interne.
- Elle ne permet pas naturellement les mécanismes multi-étapes.
- Elle ne représente pas les stratégies qui réorganisent des décisions précédentes.
- Elle ne décrit pas les algorithmes qui ont besoin d'une vue globale du problème.
- Elle risque de réduire l'évolution à une optimisation de fonctions de score.

Il faut donc introduire deux niveaux d'interface.

### A. `ScorePolicy` — interface simple de départ

Pour le premier benchmark :

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

Cette interface est stateless et permet de recombiner des expressions et des conditions locales avec une complexité limitée.

Elle doit être la première cible du crossover AST.

Les entrées sont des valeurs standardisées. Les fonctions sont déterministes et n'accèdent pas à des ressources externes.

### B. `StrategyPolicy` — interface évolutive

Le modèle doit prévoir une interface plus riche pour les futures stratégies.

Exemple conceptuel :

```rust
pub trait StrategyPolicy {
    type State;

    fn initialize(&self, context: &ProblemContext) -> Self::State;

    fn decide(
        &self,
        state: &mut Self::State,
        context: &DecisionContext,
    ) -> Decision;

    fn observe(
        &self,
        state: &mut Self::State,
        event: &ExecutionEvent,
    );
}
```

Cette interface est une cible architecturale, pas une obligation d'implémentation complète dès la première phase.

Elle permettra à terme de représenter :

- Une mémoire locale.
- Des régimes de décision.
- Des stratégies adaptatives.
- Des mécanismes conditionnels dépendant de l'historique.
- Des décisions multi-étapes.

Le modèle de données de `SkillCapsule` doit pouvoir accueillir ces différentes catégories sans migration fondamentale de l'archive.

## 4.3. Modèle `SkillCapsule`

Une capsule est une unité de compétence versionnée, immuable après validation et accompagnée de sa provenance.

Structure indicative :

```rust
pub struct SkillCapsule {
    pub id: CapsuleId,
    pub interface_version: InterfaceVersion,
    pub policy_kind: PolicyKind,
    pub source: CanonicalSource,
    pub ast_fingerprint: AstFingerprint,
    pub dependencies: Vec<DependencyRef>,
    pub provenance: GeneticProvenance,
    pub validation: ValidationStatus,
}
```

La capsule ne contient pas seulement du code.

Elle doit également conserver :

- Son interface.
- Ses dépendances.
- Ses paramètres de compilation.
- Son empreinte AST.
- Ses parents génétiques.
- Les fragments transférés.
- Les opérations de mutation.
- Les validations effectuées.
- Les résultats d'évaluation.

Une capsule publiée dans le patrimoine collectif est immuable. Une modification produit une nouvelle capsule et une nouvelle identité.

## 4.4. Extraction des unités logiques

Le crossover ne doit pas être une greffe aveugle d'un `syn::Expr` quelconque.

Le système identifie des unités logiques :

- Conditions `if/else`.
- Expressions de pénalité.
- Expressions de récompense.
- Transformations non linéaires.
- Variables intermédiaires.
- Blocs locaux.
- Sous-stratégies indépendantes.
- Fonctions auxiliaires compatibles avec l'interface.

Chaque unité extraite doit être accompagnée de son contexte de dépendances.

Il faut notamment identifier :

- Les variables utilisées.
- Les déclarations locales nécessaires.
- Les types attendus.
- Les appels auxiliaires.
- Les scopes.
- Les effets de bord éventuels.
- Les invariants du point d'insertion.

Une greffe est éligible uniquement si ses dépendances peuvent être satisfaites ou adaptées explicitement.

## 4.5. Manifeste génétique externe obligatoire

La traçabilité ne doit pas dépendre d'un commentaire source comme :

```rust
/* @nomad_graft_id: UUID */
```

Les commentaires peuvent disparaître lors de la reconstruction avec `quote`, de la normalisation du code ou d'autres transformations.

Le marqueur génétique doit être conservé dans un manifeste externe à l'AST.

Exemple :

```rust
pub struct GraftManifest {
    pub graft_id: GraftId,
    pub donor_capsule: CapsuleId,
    pub host_capsule: CapsuleId,
    pub donor_ast_hash: AstFingerprint,
    pub grafted_fragment_hash: AstFingerprint,
    pub host_locus: AstLocus,
    pub dependency_hash: DependencyHash,
    pub repair_version: Option<String>,
    pub repair_diff_hash: Option<DiffHash>,
}
```

Le manifeste doit permettre de retrouver le fragment greffé après réparation et reconstruction du programme.

La correspondance entre le fragment initial et le fragment réparé doit être vérifiable.

Si le LLM transforme tellement le fragment qu'il devient impossible d'établir cette correspondance, la greffe ne doit pas être comptabilisée comme une transmission génétique démontrée.

## 4.6. Phase 1 : crossover AST déterministe

Entrées :

- `Parent_A`
- `Parent_B`
- `recombination_seed`
- `operation_type`
- `target_locus`

Étapes :

1. Parser les parents.
2. Construire leur représentation structurelle.
3. Identifier les unités logiques candidates.
4. Analyser leurs dépendances.
5. Sélectionner un fragment donneur.
6. Vérifier la compatibilité avec le point d'insertion.
7. Réaliser la greffe déterministe.
8. Construire le manifeste génétique.
9. Calculer l'empreinte du fragment.
10. Générer le code source hybride.

Le résultat est un candidat de recombinaison, pas encore une compétence validée.

## 4.7. Phase 2 : réparation LLM sous contrainte

Si le code ne compile pas ou si des raccordements sont nécessaires, il est transmis à `nomad-operator`.

Le LLM doit recevoir :

- Le code hôte.
- Le fragment donneur.
- Le manifeste génétique.
- Les dépendances identifiées.
- Les erreurs du compilateur.
- Les contraintes d'interface.
- Les invariants à préserver.

### Prompt système de référence

> Tu es un ingénieur Rust chargé de réparer une fonction issue d'un croisement génétique AST.
>
> Le programme contient une nouvelle structure algorithmique issue d'un parent donneur. Tu dois rendre le programme compilable et conforme à son interface.
>
> Tu dois préserver le fragment greffé et son intention algorithmique. Tu peux raccorder les variables, adapter les types, déplacer des déclarations locales lorsque cela est nécessaire et corriger les incohérences syntaxiques.
>
> Tu ne dois pas supprimer, commenter, neutraliser, court-circuiter ou rendre inopérante la structure greffée.
>
> Tu ne dois pas multiplier arbitrairement une expression par zéro, écraser sa valeur ou modifier artificiellement son résultat.
>
> Si la greffe est fondamentalement irréconciliable, retourne explicitement `REPAIR_IMPOSSIBLE`.
>
> Fournis un diff explicite permettant de vérifier les modifications.

La réparation doit être contrôlée par le manifeste et par une comparaison structurelle avant/après.

Le LLM ne peut pas certifier lui-même que sa réparation respecte les contraintes.

## 4.8. Validation en trois niveaux : Anti-Dead-Code

C'est un point critique de NOMAD V2.

La simple présence d'un fragment dans l'AST ne démontre pas qu'il est actif.

La compilation ne démontre pas non plus que le fragment influence le comportement.

Enfin, une différence de décision sur quelques sondes ne démontre pas que le fragment est responsable de l'amélioration.

Le système doit distinguer trois niveaux.

### Niveau 1 — Survie structurelle

Vérifier que :

- Le fragment greffé est toujours présent.
- Son empreinte est identifiable.
- Sa structure logique n'a pas été supprimée.
- Les dépendances essentielles sont conservées.
- La réparation n'a pas remplacé le mécanisme par un équivalent vide.

### Niveau 2 — Survie computationnelle

Vérifier que le fragment est réellement exécuté et qu'il influence les valeurs intermédiaires.

Exemples :

- La condition greffée est évaluée.
- L'expression greffée contribue à une valeur de score.
- La variable calculée est utilisée dans le chemin d'exécution.
- La neutralisation du fragment modifie une valeur intermédiaire pertinente.

Les traces doivent être limitées et instrumentées afin de ne pas modifier artificiellement le comportement évalué.

### Niveau 3 — Survie comportementale

Comparer la capsule originale et une capsule d'ablation.

On définit :

- \(C\) : capsule contenant le fragment greffé.
- \(C_{\text{null}}\) : capsule dans laquelle le fragment est neutralisé.
- \(D(C)\) : décisions prises par la capsule.
- \(S(C)\) : scores et valeurs intermédiaires enregistrés.

La neutralisation doit être produite automatiquement à partir du manifeste génétique.

Elle doit préserver autant que possible le reste du programme.

Exemples :

- Condition greffée remplacée par une condition neutre.
- Terme additif neutralisé.
- Sous-expression remplacée par son équivalent neutre.
- Branche greffée remplacée par le chemin alternatif.

Le test doit utiliser plusieurs sondes et plusieurs situations pertinentes.

**Dix instances ne constituent pas une garantie suffisante.** Le nombre et la composition des sondes doivent être définis par le protocole expérimental, avec une phase de calibration et des tests complémentaires.

Les sondes doivent notamment couvrir :

- Les situations où le fragment devrait s'activer.
- Les situations où il ne devrait pas s'activer.
- Les cas limites.
- Les situations où il peut modifier un classement sans changer immédiatement la décision finale.

### Pourquoi comparer uniquement les décisions ne suffit pas

Deux capsules peuvent produire exactement le même choix de bac tout en calculant des scores différents.

Un fragment peut donc être actif computationnellement sans modifier l'argmax sur une sonde donnée.

Le test doit examiner simultanément :

- La présence structurelle.
- L'exécution du fragment.
- L'influence sur les valeurs intermédiaires.
- Les décisions finales.
- La performance sur un ensemble de sondes.

### Attribution causale de l'amélioration

Une difficulté supplémentaire doit être traitée : le LLM peut modifier d'autres parties du programme pendant la réparation.

Si la capsule réparée améliore son score, cette amélioration ne doit pas être automatiquement attribuée au fragment greffé.

Il faut comparer, lorsque cela est possible :

1. Le programme hôte initial.
2. Le programme hybride avant réparation.
3. Le programme réparé.
4. Le programme réparé avec le fragment neutralisé.
5. Éventuellement, le programme hôte ayant subi une réparation équivalente sans greffe.

Cette comparaison permet de distinguer l'effet du fragment donneur de l'effet des modifications collatérales.

Une greffe est considérée comme **fonctionnellement active** si les contrôles structurels, computationnels et comportementaux apportent des éléments convergents en sa faveur.

Une greffe active n'est pas nécessairement bénéfique. Son utilité doit encore être démontrée par l'évaluation.

## 4.9. Métriques du Pilier 2

Enregistrer :

- Syntactic Graft Survival Rate.
- Computational Graft Survival Rate.
- Behavioral Graft Survival Rate.
- Functional Graft Survival Rate.
- Taux de réparation réussie.
- Taux `REPAIR_IMPOSSIBLE`.
- Temps moyen de réparation.
- Coût LLM en tokens.
- Taux de régression.
- Taux d'amélioration attribuable au fragment.

Le GSSR doit préciser son dénominateur : greffes tentées, greffes compilées ou greffes effectivement testables. Les trois taux doivent être conservés séparément.

---

# 5. PILIER 3 — AGENTPSO ET VÉLOCITÉ SÉMANTIQUE

## 5.1. Objectif

Le rôle d'AgentPSO est de remplacer une sélection aveugle des partenaires par un routage dirigé.

Chaque organisme doit pouvoir :

- Analyser ses propres faiblesses.
- Identifier des mécanismes présents chez d'autres organismes.
- Formuler une direction d'évolution.
- Sélectionner un partenaire complémentaire.
- Évaluer empiriquement le résultat du crossover.
- Conserver l'historique des directions qui ont fonctionné.

L'objectif est d'explorer l'hypothèse selon laquelle une recombinaison guidée par la complémentarité peut être plus productive qu'un appariement aléatoire.

## 5.2. État persistant par lignée

Chaque lignée maintient une structure sérialisable :

```rust
pub struct LineageEvolutionState {
    pub p_best: CapsuleId,
    pub g_best_partner: Option<CapsuleId>,
    pub v_previous: Option<SemanticVelocity>,
    pub evolution_history: Vec<VelocityOutcome>,
}
```

- `p_best` : meilleure capsule historique de la lignée.
- `g_best_partner` : partenaire retenu pour une opération donnée.
- `v_previous` : dernière direction d'évolution utilisée.
- `evolution_history` : résultats empiriques des directions précédentes.

La mémoire directionnelle est conservée par lignée, pas uniquement dans la mémoire de travail volatile d'un agent.

## 5.3. Sélection du partenaire complémentaire

`g_best_partner` ne doit pas être systématiquement le champion global.

Le partenaire doit posséder des mécanismes susceptibles de compenser les faiblesses de l'organisme hôte.

Un score de complémentarité peut être défini :

\[
Score(B|A)=
w_1 Q(B)+
w_2 C_{\text{complement}}(A,B)+
w_3 D_{\text{AST}}(A,B)
\]

où :

- \(Q(B)\) représente la qualité du partenaire.
- \(C_{\text{complement}}\) représente la complémentarité comportementale.
- \(D_{\text{AST}}\) représente la différence structurelle.

La complémentarité doit être calculée à partir des faiblesses identifiées chez A et des points forts observés chez B.

##### Ajout oublié

Une distance AST élevée n'est pas en elle-même une preuve de complémentarité.

Les poids doivent être configurables et faire l'objet d'ablations.

## 5.4. Structure de `SemanticVelocity`

La vélocité sémantique doit être un objet structuré, persistant et exploitable par les opérateurs génétiques.

```rust
pub struct SemanticVelocity {
    pub weakness_diagnosed: String,
    pub partner_mechanism: String,
    pub source_ast_selector: AstSelector,
    pub target_insertion_locus: AstLocus,
    pub expected_behavioral_shift: String,
    pub regression_risks: Vec<String>,
    pub momentum_action: MomentumAction,
}

pub enum MomentumAction {
    ContinueDirection,
    Pivot,
    Combine,
}
```

Cette structure peut évoluer vers un format plus riche comprenant des scores de confiance, des références aux expériences ayant motivé la direction et des contraintes de validation.

Le LLM doit proposer une direction précise et vérifiable.

Il ne doit pas seulement produire une phrase du type « améliorer la gestion des espaces résiduels ».

Il doit identifier un mécanisme concret, un fragment donneur et un point d'insertion.

## 5.5. Boucle de rétroaction

Après chaque crossover, le résultat est comparé à l'attente initiale :

- Le fragment a-t-il survécu ?
- Le comportement attendu a-t-il changé ?
- La fitness s'est-elle améliorée ?
- Une nouvelle niche a-t-elle été découverte ?
- Une régression a-t-elle été introduite ?
- La direction précédente mérite-t-elle d'être poursuivie ?

L'organisme peut alors :

- Continuer la direction.
- Changer de direction.
- Combiner plusieurs mécanismes.
- Abandonner une piste.

## 5.6. Évaluation d'AgentPSO

Comparer :

- Appariement aléatoire.
- Appariement fondé uniquement sur la fitness.
- Appariement fondé sur la diversité structurelle.
- Appariement AgentPSO fondé sur la complémentarité.

Mesurer :

- Gain moyen par crossover.
- Taux de crossovers productifs.
- Taux de nouvelles niches.
- Taux de greffes fonctionnellement actives.
- Nombre d'évaluations nécessaires pour atteindre un seuil.
- Coût LLM supplémentaire.

AgentPSO n'est considéré comme utile que s'il apporte un gain mesurable par rapport à des stratégies de sélection plus simples, à budget comparable.

---

# 6. PILIER 4 — ÉVALUATION DYNAMIQUE ET tLDR

## 6.1. Objectif

L'évaluation ne doit pas se limiter au score final.

Deux algorithmes peuvent produire le même nombre de bacs à la fin d'une instance tout en ayant des comportements intermédiaires très différents.

NOMAD doit donc enregistrer des trajectoires d'exécution et étudier si certaines caractéristiques dynamiques sont associées à une meilleure robustesse ou à une meilleure généralisation.

Le tLDR est une hypothèse de métrique expérimentale.

Il ne doit pas être présenté comme une propriété mathématique déjà démontrée.

## 6.2. Environnement d'exécution Wasmtime

Les capsules sont compilées vers WebAssembly et exécutées dans un environnement Wasmtime isolé.

Objectifs :

- Isolation du code généré.
- Limitation des ressources.
- Exécution contrôlée.
- Reproductibilité dans un environnement versionné.
- Mesure du coût d'exécution.
- Protection du processus orchestrateur.

Le *fuel metering* de Wasmtime fournit une comptabilisation d'unités de fuel associées à l'exécution WebAssembly.

**Ce n'est pas un comptage exact des cycles CPU physiques.**

Le fuel constitue une mesure de coût logique reproductible dans un environnement donné. Le temps réel, la consommation mémoire et les autres coûts doivent être mesurés séparément.

## 6.3. Enregistrement des trajectoires

À chaque pas \(t\), ou à des intervalles configurables, enregistrer :

- Nombre de bacs ouverts \(B(t)\).
- Somme des tailles insérées.
- Distribution des espaces résiduels.
- Entropie des espaces résiduels.
- Nombre de décisions prises.
- Coût fuel cumulé.
- Événements significatifs de l'algorithme.

Les trajectoires complètes peuvent être conservées pour les capsules élites et les expériences ciblées.

Les autres évaluations peuvent conserver des résumés statistiques.

## 6.4. Définition du potentiel expérimental

Le nombre de bacs \(B(t)\) augmente ou reste constant au fil des insertions. Il ne constitue donc pas directement une fonction de Lyapunov décroissante.

On définit un écart relatif à une borne inférieure volumétrique préfixe :

\[
L_{\text{vol}}(t)=
\left\lceil
\frac{\sum_{i=1}^{t}w_i}{C}
\right\rceil
\]

où :

- \(w_i\) est la taille du \(i\)-ème objet.
- \(C\) est la capacité d'un bac.

Cette quantité est une borne inférieure volumétrique simple. **Elle ne doit pas être appelée relaxation complète de Martello-Toth \(L_1\)**.

On définit ensuite :

\[
V(t)=
\frac{B(t)-L_{\text{vol}}(t)}
{L_{\text{vol}}(t)}
\]

Cette quantité représente l'écart relatif entre le nombre de bacs utilisés et la borne volumétrique préfixe.

Elle est non négative lorsque les hypothèses du problème sont respectées.

Cependant, **il n'existe aucune garantie générale que \(V(t)\) décroisse au fil du temps**.

Sa trajectoire dépend :

- De la distribution des tailles.
- De l'ordre des objets.
- De la capacité des bacs.
- De la stratégie d'insertion.
- Des changements de régime dans le flux d'objets.

La pente observée peut donc refléter la composition de la séquence plutôt que la qualité intrinsèque de l'algorithme.

## 6.5. Définition expérimentale du tLDR

Le tLDR est conservé comme indicateur expérimental de dynamique.

Sur une fenêtre \(t \in [t_0,N]\), avec une valeur initiale proposée :

\[
t_0=\lfloor N/5 \rfloor
\]

On définit :

\[
y(t)=\ln(V(t)+\epsilon)
\]

avec :

\[
\epsilon=10^{-4}
\]

On effectue une régression linéaire de \(y(t)\) sur le temps normalisé :

\[
\tau=t/N
\]

Soient :

- \(\beta_1\) : pente de régression.
- \(\sigma_{\text{res}}\) : écart-type des résidus.

Le score expérimental proposé est :

\[
tLDR=-\beta_1-\lambda\sigma_{\text{res}}
\]

avec une valeur initiale de :

\[
\lambda=0.5
\]

Une valeur élevée signifie que le potentiel mesuré diminue rapidement et de façon relativement régulière sur la fenêtre observée.

Cela ne signifie pas nécessairement que l'algorithme généralise mieux.

### Cas particuliers à traiter

Le calcul doit définir explicitement :

- Les préfixes pour lesquels la borne vaut 1.
- Les trajectoires avec \(V(t)=0\).
- Les trajectoires presque constantes.
- Les instances trop courtes.
- Les séquences où le potentiel augmente.
- Les situations où la régression est instable.

Le système doit conserver les valeurs brutes et les paramètres du calcul pour permettre une réanalyse ultérieure.

## 6.6. Validation scientifique du tLDR

Avant de l'utiliser comme critère de sélection, il faut tester sa pertinence.

Expériences obligatoires :

1. Comparaison sur plusieurs distributions de tailles.
2. Comparaison sur des séquences de même composition mais d'ordres différents.
3. Comparaison avec des heuristiques connues.
4. Étude de sensibilité à \(t_0\).
5. Étude de sensibilité à \(\lambda\).
6. Comparaison avec des variantes de potentiel.
7. Mesure de la corrélation entre tLDR et performance finale.
8. Mesure du pouvoir prédictif du tLDR sur les résultats OOD.

Une métrique qui ne prédit pas mieux la généralisation que la fitness finale ne doit pas être conservée comme signal de sélection sous prétexte qu'elle est mathématiquement élégante.

Le nom *Trajectory-aware Lyapunov Decay Rate* est conservé comme nom de travail historique. Il ne constitue pas une démonstration de propriété de Lyapunov.

## 6.7. Sélection multi-objectifs

La fitness finale reste le critère principal.

Le tLDR peut servir :

- D'indicateur secondaire.
- De critère de départage.
- De variable d'analyse.
- Éventuellement de deuxième objectif si les expériences le justifient.

Il ne doit pas remplacer le nombre de bacs.

Le coût fuel peut également être pris en compte.

La politique de sélection doit être explicite et testée par ablation.

## 6.8. Isolation des données OOD

Les jeux OOD sont sanctuarisés.

Ils ne doivent jamais être utilisés pour :

- Choisir les parents.
- Modifier les poids de sélection.
- Générer les vélocités.
- Ajuster les prompts.
- Modifier les stratégies pendant l'évolution.

Ils servent exclusivement à l'évaluation indépendante.

---

# 7. PILIER TRANSVERSAL — ACCUMULATION CUMULATIVE DES COMPÉTENCES

## 7.1. Objectif scientifique central

C'est le cœur du projet NOMAD.

L'hypothèse est que des organismes successifs peuvent progresser plus rapidement et développer des compétences plus élaborées lorsqu'ils héritent d'un patrimoine collectif validé.

La démonstration doit aller au-delà de la simple persistance d'un fichier ou d'une stratégie.

Il faut démontrer que :

1. Une génération découvre une compétence.
2. Cette compétence est conservée dans le patrimoine collectif.
3. Une nouvelle génération peut l'utiliser sans connaître l'histoire de son créateur.
4. Elle peut la recombiner avec d'autres compétences.
5. Le résultat apporte un avantage mesurable.
6. Cet avantage subsiste après destruction des mémoires individuelles.

C'est cette chaîne qui constitue le phénomène de transmission cumulative à étudier.

## 7.2. Trois métriques principales

### A. Innovation Rate

\[
R_{\text{innov}}=
\frac{\text{Innovations validées}}
{\text{Nombre d'évaluations}}
\times 1000
\]

Une innovation peut être :

- Une nouvelle niche comportementale.
- Un nouveau motif structurel AST.
- Une nouvelle combinaison de fragments.
- Une amélioration fonctionnelle d'une compétence existante.

Il faut distinguer les innovations syntaxiques des innovations comportementales.

Une nouvelle structure AST qui n'a aucun effet fonctionnel ne compte pas comme innovation fonctionnelle.

### B. Functional Heritability Rate

Mesure la proportion des innovations fonctionnelles introduites à une génération qui sont encore présentes et fonctionnellement actives dans les générations suivantes.

\[
R_{\text{herit}}=
\frac{\text{Motifs innovants transmis et actifs}}
{\text{Motifs innovants initiaux}}
\]

L'évaluation doit porter sur des motifs identifiés par leur provenance et validés par ablation différentielle.

Une simple présence syntaxique ne suffit pas.

### C. Cumulative Improvement Factor

Le CIF mesure l'avantage d'une population héritière par rapport à une population repartant de zéro.

Une première définition peut utiliser le rapport des budgets d'évaluations nécessaires pour atteindre un seuil fixé :

\[
CIF=
\frac{Budget_{\text{Tabula Rasa}}}
{Budget_{\text{Héritière}}}
\]

Un CIF supérieur à 1 indique un avantage de la population héritière.

Il faut également enregistrer les courbes complètes de convergence et leur aire sous la courbe, afin de ne pas dépendre d'un seul seuil de performance.

Les seuils doivent être définis avant les expériences.

## 7.3. Test de mémoire collective : Mort et Renaissance

### Époque 1 — Genèse

Une population \(P_1\) évolue pendant un nombre défini de générations avec un budget donné.

Les compétences validées sont enregistrées dans `nomad-archive`.

Le patrimoine conserve :

- Les capsules.
- Les dépendances.
- Les empreintes.
- Les métadonnées.
- Les résultats de validation.
- Les liens de provenance.
- Les relations de recombinaison.

Il ne doit pas conserver par défaut les mémoires individuelles nécessaires à la reproduction artificielle d'un agent particulier.

### Extinction

Destruction complète :

- Des organismes de \(P_1\).
- Des mémoires de travail.
- Des états PSO individuels.
- Des historiques de réflexion privés.
- Des caches individuels de prompts.

Le patrimoine collectif validé est conservé.

### Époque 2 — Héritage contre Tabula Rasa

Lancer deux populations indépendantes.

**Population A — Héritière**

- Organismes vierges.
- Accès en lecture au patrimoine validé de l'époque 1.
- Possibilité de recombiner les compétences héritées.
- Aucun accès aux mémoires individuelles de \(P_1\).

**Population B — Tabula Rasa**

- Organismes vierges.
- Heuristiques initiales identiques.
- Aucun accès au patrimoine collectif de l'époque 1.

Les deux populations doivent avoir :

- Le même budget LLM.
- Le même budget CPU.
- Le même nombre d'évaluations.
- Les mêmes modèles et versions.
- Les mêmes données d'entraînement.
- Les mêmes conditions d'initialisation, à l'exception du patrimoine transmis.

Le protocole doit éviter que la population héritière bénéficie simplement d'un plus grand nombre de tokens ou d'évaluations.

## 7.4. Expériences complémentaires de transmission

Pour isoler les mécanismes responsables d'un éventuel avantage, comparer :

1. Héritage des capsules seules.
2. Héritage des capsules et de leurs dépendances.
3. Héritage des capsules avec leur provenance.
4. Héritage des capsules avec leurs métadonnées comportementales.
5. Absence complète d'héritage.

Ces expériences permettront de déterminer ce qui apporte réellement de la valeur : le code, les métadonnées, les relations généalogiques ou la combinaison de ces éléments.

## 7.5. Conservation du patrimoine

Le patrimoine ne doit pas être une simple collection des meilleurs programmes.

Il doit préserver :

- Les compétences validées.
- Les variantes complémentaires.
- Les mécanismes historiquement utiles.
- Les dépendances.
- Les relations généalogiques.
- Les résultats positifs et négatifs des croisements.

Une compétence sous-optimale isolément peut être très utile comme donneur génétique.

Il ne faut donc pas purger systématiquement toutes les capsules qui ne sont pas championnes de leur benchmark.

---

# 8. PROTOCOLE SCIENTIFIQUE ET TESTS D'ACCEPTATION

## 8.1. Principes généraux

Chaque expérience doit être reproductible.

Les paramètres suivants doivent être enregistrés :

- Seed.
- Version du code.
- Version de Rust.
- Version de Wasmtime.
- Version des dépendances.
- Configuration de compilation.
- Version du modèle LLM.
- Paramètres du modèle.
- Budgets CPU et LLM.
- Datasets utilisés.
- Paramètres de l'évaluateur.

Les tests doivent utiliser des seeds indépendantes.

Cinq seeds peuvent servir à une première exploration, mais ne constituent pas une base statistique suffisante pour une conclusion forte.

Protocole recommandé :

- 10 seeds au minimum pour une campagne préliminaire.
- 30 seeds si le budget le permet pour les comparaisons principales.
- Augmentation du nombre de seeds si la variabilité observée est importante.

Les comparaisons appariées doivent utiliser des seeds et des conditions comparables lorsque cela est pertinent.

Rapporter :

- Médiane.
- Moyenne lorsque pertinente.
- Écart interquartile.
- Intervalles de confiance.
- Tailles d'effet.
- Résultats des tests statistiques.
- Nombre d'expériences réalisées.

Le choix du test statistique doit correspondre au plan expérimental. Mann-Whitney U peut être utilisé pour des groupes indépendants ; Wilcoxon peut être pertinent pour des comparaisons appariées.

En cas de comparaisons multiples, appliquer une correction appropriée.

Un résultat non significatif doit être rapporté comme tel, et non transformé en succès narratif.

## 8.2. TEST 1 — BREAKING THE 1265 CEILING

**Objectif :** démontrer que le crossover AST permet de produire une amélioration structurelle au-delà du meilleur résultat de référence V1.

Conditions :

- Dataset ID de référence V1.
- Budget LLM plafonné à 10 000 appels pour la campagne de référence, à confirmer selon la définition exacte d'un appel.
- Comparaison avec V1.
- Comparaison avec une mutation LLM libre sans opérateur AST.
- Budgets CPU et LLM comparables.
- Plusieurs seeds.

Critères :

1. Une lignée atteint moins de 1265 bacs sur le benchmark de référence.
2. Le résultat est reproductible sur plusieurs seeds.
3. Le mécanisme structurel est identifiable.
4. Le fragment greffé passe les contrôles structurels, computationnels et comportementaux.
5. L'ablation du fragment permet de mesurer son influence.
6. La provenance des deux parents est traçable.
7. Le gain est attribuable au crossover AST et non à une modification collatérale du LLM.
8. Le résultat ne dépend pas d'une fuite du dataset OOD.

### Anti-Feature-Engineering

Les prompts système ne doivent pas révéler artificiellement le mécanisme que l'expérience cherche à faire émerger.

En particulier, ne pas fournir directement :

- Le seuil numérique recherché.
- Le concept exact de pénalité attendu.
- La structure conditionnelle que l'on espère découvrir.

Les organismes peuvent naturellement découvrir des structures proches de ces mécanismes. C'est précisément l'objet de l'expérience.

Le résultat doit être décrit comme une découverte expérimentale dans ce système, pas comme une preuve d'invention totalement indépendante de toute influence humaine.

## 8.3. TEST 2 — RÉSILIENCE GÉNÉTIQUE ET QUALITY-DIVERSITY

Objectif : vérifier que MAP-Elites maintient des familles comportementales distinctes.

Comparer avec la sélection Top 20 % de V1.

Mesurer à la génération 15 et sur plusieurs générations :

- Couverture des niches.
- Diversité comportementale.
- Diversité structurelle.
- Descendance des niches.
- Contribution des lignées minoritaires.
- Performance globale.

Le seuil de 30 % de couverture et les cinq clusters distincts sont des objectifs initiaux, pas des vérités scientifiques.

Ils doivent être considérés comme des seuils opérationnels à calibrer.

La distance AST moyenne supérieure à 0,40 ne peut être retenue qu'après définition et normalisation précise de la métrique.

Critère particulièrement important :

Au moins une élite finale doit pouvoir être reliée à une lignée issue d'un organisme qui n'était pas dans le Top 20 % global à la génération 5.

Cela permet de tester l'intérêt des lignées « tremplins ».

## 8.4. TEST 3 — POUVOIR PRÉDICTIF DU tLDR SUR OOD

Objectif : déterminer si le tLDR apporte une information prédictive complémentaire à la fitness finale.

Benchmarks OOD envisagés :

- OOD-1 : micro-objets.
- OOD-2 : distributions de type Pareto ou loi de puissance.
- OOD-3 : objets fortement asymétriques.
- OOD-4 : séquences triées ou adverses.

Comparer trois types de champions :

1. Champion fitness finale.
2. Champion tLDR.
3. Champion Pareto fitness + tLDR.

Critères :

- Performance finale OOD.
- Dégradation relative par rapport à ID.
- Variabilité entre distributions.
- Taille d'effet.
- Pouvoir prédictif du tLDR au-delà de la fitness finale.

Le test doit aussi inclure des séquences de même composition présentées dans des ordres différents.

Si le tLDR n'apporte pas de pouvoir prédictif supplémentaire, le résultat doit être documenté négativement.

## 8.5. TEST 4 — TRANSMISSION DU PATRIMOINE COLLECTIF

Objectif : valider le protocole Mort et Renaissance.

Critères :

1. La population héritière atteint un seuil de performance avec un budget inférieur à la population Tabula Rasa.
2. Les motifs hérités restent fonctionnellement actifs après plusieurs générations.
3. Des recombinaisons entre compétences héritées produisent de nouveaux descendants viables.
4. L'avantage subsiste après destruction des mémoires individuelles.
5. Les différences de budget, de modèles et de ressources sont contrôlées.

L'objectif initial de réduction de 40 % du budget d'évaluations peut être conservé comme cible ambitieuse.

Il ne doit pas être présenté comme une condition scientifique universelle de réussite.

Le seuil de 25 % d'héritabilité fonctionnelle après cinq générations est également une cible expérimentale à justifier par la calibration et la puissance statistique.

La démonstration repose sur l'effet mesuré, pas sur l'atteinte arbitraire d'un pourcentage.

---

# 9. MATRICE D'EXPÉRIENCES D'ABLATION

Chaque composant doit démontrer sa valeur ajoutée marginale.

Les budgets de calcul et LLM doivent être comparables.

| Groupe | MAP-Elites | AST Crossover | AgentPSO | tLDR | Objectif |
|---|---|---|---|---|---|
| A — Baseline V1 | Non | Non | Non | Non | Référence historique |
| B — QD Only | Oui | Non | Non | Non | Impact de la diversité |
| C — AST Only | Non | Oui | Non | Non | Impact du crossover structurel |
| D — QD + AST | Oui | Oui | Non | Non | Synergie diversité et recombinaison |
| E — QD + AST + PSO | Oui | Oui | Oui | Non | Valeur du routage complémentaire |
| F — V2 Full | Oui | Oui | Oui | Oui | Évaluation globale |

## 9.1. Sous-ablations du crossover

Comparer :

- Greffe AST brute.
- Greffe AST avec réparation LLM.
- Mutation LLM libre sans greffe.
- Greffe AST avec réparation et validation complète.
- Greffe AST sans filtre Anti-Dead-Code.

Les tests doivent mesurer le taux de greffes actives, le taux d'amélioration et le coût.

## 9.2. Sous-ablations MAP-Elites

Comparer :

- Sélection Top 20 %.
- MAP-Elites comportemental 2D.
- MAP-Elites comportemental 3D.
- CVT-MAP-Elites.
- MAP-Elites avec et sans curiosité.

## 9.3. Sous-ablations AgentPSO

Comparer :

- Partenaire aléatoire.
- Partenaire meilleur fitness.
- Partenaire structurellement distant.
- Partenaire complémentaire choisi par AgentPSO.

## 9.4. Sous-ablations tLDR

Comparer :

- Fitness finale seule.
- Fitness + tLDR.
- Fitness + coût fuel.
- Fitness + tLDR + coût fuel.

## 9.5. Ablation de l'héritage

Comparer :

- Capsules héritées.
- Capsules héritées sans métadonnées.
- Capsules héritées avec provenance.
- Population sans patrimoine.
- Population avec patrimoine aléatoire de contrôle, si pertinent.

Cette matrice doit permettre de comprendre quels mécanismes contribuent réellement à l'accumulation cumulative.

---

# 10. EXIGENCES TECHNIQUES RUST

## 10.1. Robustesse et isolation

Une capsule ne doit jamais pouvoir faire tomber le processus `nomad-orchestrator`.

Les erreurs possibles doivent être traitées :

- Panics.
- Dépassements d'entiers.
- Divisions par zéro.
- Boucles infinies.
- Consommation mémoire excessive.
- Erreurs de compilation.
- Erreurs de désérialisation.
- Entrées malformées.
- Erreurs d'exécution WebAssembly.

Les capsules sont exécutées dans une instance Wasmtime isolée.

Configuration de sécurité initiale :

- Limite de mémoire linéaire, par exemple 16 Mo, à calibrer selon le workload.
- Limite de fuel.
- Absence d'accès WASI au système de fichiers.
- Absence d'accès réseau.
- Absence d'accès à l'horloge.
- Absence de ressources hôte non autorisées.

L'isolation in-process de Wasmtime protège contre de nombreux comportements du code invité, mais ne remplace pas l'isolation du processus hôte face aux vulnérabilités du runtime ou de ses dépendances.

Les campagnes expérimentales doivent donc également prévoir un processus d'évaluation isolé avec limites de ressources OS.

## 10.2. Compilation WebAssembly

La cible initiale est :

`wasm32-unknown-unknown`

Le pipeline de compilation doit être prototypé avant d'être figé.

L'utilisation directe de `rustc` sans Cargo ne doit pas être considérée comme acquise pour un projet utilisant de véritables dépendances Rust.

Il faut comparer :

1. Compilation directe `rustc` avec dépendances préparées.
2. Compilation via Cargo avec environnement verrouillé.
3. Compilation incrémentale et cache de dépendances.
4. Compilation en répertoire temporaire RAM lorsque cela apporte un gain mesurable.

`tmpfs` réduit principalement les coûts d'entrées-sorties. Il ne supprime pas le coût CPU de compilation.

Le choix doit être fait à partir de mesures réelles.

## 10.3. Reproductibilité déterministe

Tous les opérateurs évolutionnaires doivent utiliser des générateurs aléatoires initialisés par les seeds de l'expérience.

`thread_rng()` et les sources aléatoires non contrôlées sont interdites dans le chemin expérimental.

Un générateur tel que `ChaCha8Rng` peut être utilisé.

Les expériences doivent enregistrer :

- Seed initiale.
- Seeds dérivées.
- Ordre des opérations.
- Version du compilateur.
- Version du runtime.
- Paramètres de compilation.
- Versions des dépendances.

**La reproductibilité bit-à-bit universelle entre toutes les machines ne doit pas être promise.**

Elle doit être recherchée et testée dans un environnement versionné.

Les résultats peuvent dépendre de :

- Versions de Rust.
- Versions de Wasmtime.
- Options de compilation.
- Opérations flottantes.
- Architecture matérielle.
- Dépendances.
- Ordonnancement.
- Comportement des modèles LLM.

La cible réaliste est une reproductibilité stricte dans un environnement contrôlé, accompagnée de tests de reproductibilité intermachines.

## 10.4. Déduplication et cache

Le hash AST seul ne suffit pas pour garantir qu'une capsule peut réutiliser un résultat d'évaluation.

Le cache doit prendre en compte :

- AST canonique.
- Interface et version d'interface.
- Dépendances et versions.
- Configuration de compilation.
- Version de Rust.
- Version de Wasmtime.
- Configuration du runtime.
- Paramètres de l'évaluateur.
- Version du benchmark.
- Dataset et seed.

Le cache doit distinguer :

- Cache de parsing.
- Cache de compilation.
- Cache d'exécution.
- Cache de résultats expérimentaux.

Une même AST peut produire des résultats différents dans des environnements ou avec des dépendances différentes.

## 10.5. Fingerprint AST

Le fingerprint doit être calculé sur une représentation canonique.

La normalisation peut inclure :

- Suppression des commentaires.
- Normalisation des identifiants locaux lorsque cela est sûr.
- Normalisation des espaces et formats.
- Représentation stable des opérateurs et des nœuds.
- Inclusion des informations de dépendances pertinentes.

La normalisation ne doit pas supprimer des distinctions sémantiques.

Le hash est un outil d'identification et de déduplication, pas une preuve d'équivalence comportementale.

## 10.6. Stockage

Le stockage doit être durable et permettre de retrouver l'historique d'une expérience.

SQLite constitue un choix initial raisonnable pour :

- Les métadonnées.
- Les capsules.
- Les générations.
- Les relations généalogiques.
- Les résultats expérimentaux.
- Les index de l'archive.

Le choix définitif du moteur de stockage doit rester indépendant des structures métier.

## 10.7. Sécurité du LLM

Le LLM ne doit jamais être considéré comme une autorité de validation.

Tout code produit ou réparé doit être :

- Parsé.
- Contrôlé.
- Compilé.
- Exécuté en sandbox.
- Évalué.
- Comparé à une variante d'ablation lorsque nécessaire.

Les prompts et les sorties doivent être enregistrés pour les campagnes reproductibles, dans le respect des contraintes de confidentialité applicables.

---

# 11. PLAN DE DÉVELOPPEMENT SÉQUENTIEL

Le développement doit être incrémental.

Il faut éviter de construire simultanément tous les piliers sans avoir validé les fondations.

## Phase 0 — Audit de NOMAD V1 et référence scientifique

Objectifs :

- Cartographier les crates existantes.
- Identifier les interfaces actuelles.
- Comprendre le fonctionnement du Swarm Crossover.
- Reproduire le résultat de référence de 1265 bacs.
- Identifier les seeds et les benchmarks disponibles.
- Construire un harnais de non-régression.
- Définir les budgets expérimentaux.
- Documenter les limites de V1.

Livrable : rapport d'audit et protocole de référence.

Aucune modification majeure ne doit être engagée avant cet audit.

## Phase 1 — Sandbox et pipeline d'évaluation

Objectifs :

- Prototyper la compilation WebAssembly.
- Intégrer Wasmtime.
- Mesurer le coût de compilation.
- Mesurer le coût fuel.
- Tester les limites mémoire.
- Vérifier l'isolation.
- Définir les structures de trajectoire.
- Vérifier la reproductibilité.

Le tLDR peut être prototypé à ce stade, mais ne doit pas encore influencer la sélection.

Livrable : évaluateur indépendant et sécurisé.

## Phase 2 — Modèle de capsule extensible

Objectifs :

- Définir `SkillCapsule`.
- Introduire `ScorePolicy` pour le Bin Packing.
- Prévoir l'évolution vers `StrategyPolicy`.
- Définir les interfaces et versions.
- Définir le manifeste génétique.
- Définir les fingerprints.
- Définir la provenance.

Livrable : modèle de compétence stable et versionné.

## Phase 3 — MAP-Elites comportemental

Objectifs :

- Implémenter les descripteurs.
- Implémenter la grille 2D/3D.
- Implémenter le mode CVT.
- Remplacer la sélection Top 20 %.
- Mesurer la couverture.
- Mesurer la diversité comportementale.
- Vérifier la conservation des lignées minoritaires.

Livrable : archive Quality-Diversity opérationnelle.

## Phase 4 — Crossover AST et provenance

Objectifs :

- Intégrer `syn` et `quote`.
- Identifier les unités logiques.
- Analyser les dépendances.
- Réaliser des greffes déterministes.
- Construire le manifeste génétique.
- Identifier précisément les fragments transférés.
- Implémenter les premières mutations structurelles.

Livrable : crossover AST déterministe sans réparation LLM obligatoire.

## Phase 5 — Réparation LLM et validation différentielle

Objectifs :

- Implémenter les prompts de réparation.
- Préserver le manifeste génétique.
- Comparer les AST avant et après réparation.
- Générer les variantes d'ablation.
- Implémenter les contrôles structurels.
- Implémenter les contrôles computationnels.
- Implémenter les sondes comportementales.
- Mesurer le taux de survie fonctionnelle.

Livrable : pipeline de greffe et de validation complet.

## Phase 6 — AgentPSO et vélocité sémantique

Objectifs :

- Définir `SemanticVelocity`.
- Implémenter le diagnostic de faiblesse.
- Identifier les partenaires complémentaires.
- Conserver l'historique des directions.
- Comparer avec l'appariement aléatoire.
- Mesurer le gain marginal.

Livrable : routage évolutionnaire dirigé.

## Phase 7 — tLDR et généralisation

Objectifs :

- Stabiliser les trajectoires.
- Calculer les variantes de potentiel.
- Implémenter le tLDR expérimental.
- Tester la sensibilité à l'ordre.
- Tester les distributions synthétiques.
- Comparer les métriques de trajectoire.
- Préparer les benchmarks OOD.
- Évaluer le pouvoir prédictif.

Livrable : étude de validité du tLDR.

## Phase 8 — Patrimoine collectif

Objectifs :

- Finaliser `nomad-archive`.
- Stocker les capsules validées.
- Gérer les DAG de provenance.
- Implémenter les métriques d'innovation.
- Implémenter l'héritabilité fonctionnelle.
- Implémenter le CIF.
- Construire le protocole Mort et Renaissance.
- Réaliser les premières expériences d'héritage.

Livrable : système de patrimoine collectif opérationnel.

## Phase 9 — Campagne scientifique

Objectifs :

- Exécuter la matrice d'ablation.
- Exécuter les quatre tests d'acceptation.
- Réaliser les expériences multi-seeds.
- Produire les intervalles de confiance.
- Calculer les tailles d'effet.
- Documenter les résultats positifs et négatifs.
- Évaluer les limites et les biais possibles.

Livrable : rapport scientifique complet sur NOMAD V2.

**Pas de développement P2P dans cette version.** L'accumulation culturelle doit d'abord être démontrée sur une machine unique.

---

# 12. PRINCIPES NON NÉGOCIABLES

1. Ne jamais confondre diversité syntaxique et diversité comportementale.
2. Ne jamais construire une grille MAP-Elites de haute dimension non bornée.
3. Ne jamais faire de `score_bin` la limite définitive de l'espace des compétences.
4. Ne jamais demander à `syn` de résoudre le typage ou le borrow checking Rust.
5. Ne jamais effectuer une greffe AST sans analyser ses dépendances.
6. Ne jamais considérer une réparation LLM compilable comme une greffe réussie.
7. Ne jamais dépendre uniquement de commentaires pour tracer une greffe génétique.
8. Ne jamais confondre survie structurelle, survie computationnelle et survie comportementale.
9. Ne jamais attribuer automatiquement au fragment greffé un gain provoqué par des modifications collatérales du LLM.
10. Ne jamais considérer le tLDR comme une fonction de Lyapunov démontrée.
11. Ne jamais utiliser le tLDR comme critère principal avant validation expérimentale.
12. Ne jamais utiliser les datasets OOD pour orienter l'évolution.
13. Ne jamais déclarer une supériorité scientifique sur une seule seed.
14. Ne jamais promettre une reproductibilité bit-à-bit universelle sans preuve.
15. Ne jamais utiliser un cache AST sans prendre en compte l'environnement d'exécution.
16. Ne jamais confondre conservation d'une capsule et transmission fonctionnelle d'une compétence.
17. Ne jamais développer la couche P2P avant validation de l'accumulation collective sur une machine unique.
18. Ne jamais considérer une hypothèse séduisante comme un résultat démontré.

---

# 13. INSTRUCTIONS INITIALES À L'IA DÉVELOPPEUSE

Avant toute modification du dépôt DARWIN.RS :

## Étape 1 — Audit

Auditer intégralement le dépôt :

- Crates existantes.
- Structures de données.
- Cycle de vie des organismes.
- Méthode d'évaluation.
- Swarm Crossover.
- Gestion des générations.
- Stockage.
- Dépendances.
- Points de couplage.

Identifier les éléments réutilisables et ceux qui doivent être remplacés.

## Étape 2 — Rapport de migration

Présenter un rapport comprenant :

- Architecture actuelle.
- Architecture cible.
- Différences.
- Risques techniques.
- Coûts estimés.
- Dépendances à ajouter.
- Interfaces Rust proposées.
- Ordre de migration.

Les structures Rust exactes suivantes doivent être proposées avant implémentation :

- `SkillCapsule`
- `ScorePolicy`
- `StrategyPolicy` (interface cible)
- `GraftManifest`
- `GeneticProvenance`
- `MapElitesArchive`
- `BehaviorDescriptor`
- `SemanticVelocity`
- `TrajectoryMetrics`
- `ValidationStatus`

## Étape 3 — Prototypage des risques majeurs

Avant de développer l'ensemble du système, réaliser trois prototypes techniques :

1. Compilation réelle d'une capsule Rust vers WebAssembly avec ses dépendances.
2. Greffe AST d'un fragment conditionnel entre deux fonctions, avec compilation et manifeste génétique.
3. Neutralisation automatique du fragment et comparaison des valeurs intermédiaires et des décisions.

Ces prototypes doivent permettre de vérifier que les trois fondations les plus risquées sont techniquement réalisables.

## Étape 4 — Développement incrémental

Procéder phase par phase.

Chaque phase doit :

- Compiler avec `cargo check --workspace`.
- Passer les tests unitaires.
- Passer les tests d'intégration.
- Produire des métriques.
- Être documentée.
- Être reproductible.

Ne pas commencer la phase suivante tant que les critères de validation de la phase actuelle ne sont pas satisfaits.

## Étape 5 — Esprit critique obligatoire

Cette spécification est un cadre évolutif, pas une vérité intangible.

Si l'analyse du code réel, les prototypes ou les expériences révèlent :

- Une incohérence architecturale.
- Un coût prohibitif.
- Une impossibilité technique.
- Une hypothèse scientifique invalide.
- Un biais expérimental.
- Un mécanisme inutile.

L'IA développeuse doit le signaler explicitement, expliquer le problème et proposer une correction argumentée.

Elle ne doit jamais implémenter aveuglément une spécification simplement parce qu'elle y est écrite.

---

# 14. CRITÈRE FINAL DE RÉUSSITE DE NOMAD V2

NOMAD V2 ne sera pas considéré comme réussi simplement parce que :

- Le code compile.
- Le moteur ne plante pas.
- MAP-Elites conserve plusieurs niches.
- Le crossover AST fonctionne.
- Un organisme obtient un score inférieur à 1265.
- Le tLDR présente une corrélation intéressante.

Ces résultats sont des étapes intermédiaires.

La démonstration centrale doit être la suivante :

**Une population d'organismes artificiels est capable de produire des compétences nouvelles, de les conserver dans un patrimoine collectif, de les transmettre à des organismes vierges, puis de les recombiner pour produire de nouvelles compétences, avec un avantage cumulatif mesurable par rapport à une population repartant de zéro.**

Cette démonstration doit résister :

- À la destruction des mémoires individuelles.
- À des budgets de calcul comparables.
- À des contrôles sans crossover.
- À des contrôles sans patrimoine.
- À des évaluations indépendantes.
- À plusieurs seeds.
- À des tests de généralisation.

Si NOMAD parvient à démontrer cela, il aura établi quelque chose de bien plus intéressant qu'une nouvelle heuristique de Bin Packing.

Il aura fourni une première démonstration expérimentale d'un mécanisme d'accumulation culturelle artificielle fondé sur l'évolution, la recombinaison et la transmission de compétences.

**La priorité absolue de NOMAD V2 est donc de démontrer la transmission cumulative fonctionnelle, et non de maximiser prématurément la complexité de son architecture.**