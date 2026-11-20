# NOMAD Grid — Architecture de la grille (brouillon v0)

Statut : document de conception. Aucun code réseau n'existe encore dans le dépôt.
Ce document ne modifie aucun crate existant.

## 1. Objectif

Permettre à des machines volontaires, sur Internet, de contribuer à une même recherche
évolutionnaire : chaque machine génère des heuristiques avec son propre LLM local, et
toutes partagent une archive commune des meilleures.

Ce que la grille apporte :

- **Plus de générations LLM.** C'est le calcul coûteux (plusieurs secondes par appel sur
  GPU). L'évaluation d'une heuristique est bien moins chère.
- **Des modèles différents.** Un nœud sous Qwen, un autre sous DeepSeek ou Gemma
  produisent des populations différentes, que le croisement AST peut recombiner.

Ce que la grille n'apporte pas : elle ne corrige pas une recherche qui plafonne. Elle n'a
d'intérêt que si l'évolution bat les baselines classiques sur un seul nœud.

## 2. Ce qui existe déjà et sert de fondation

| Propriété | Où | Rôle dans la grille |
|---|---|---|
| Exécution sans compilation | `nomad-sandbox` (interpréteur AST, fuel, profondeur 48) | Exécuter le code d'un inconnu sans risque |
| Jeux de données déterministes | `nomad-evaluator` (ChaCha8 + seed, hash SHA-256) | Tout nœud recalcule le même score |
| Capsule sans score | `nomad-core::SkillCapsule` | Un score annoncé n'est jamais cru, il est recalculé |
| Identifiant par contenu | `SkillCapsule::new_v2` (SHA-256) | Déduplication et échange |
| Audit de génération | `ReproducibilityAudit` | Traçabilité du modèle et du prompt |

## 3. Modèle de menace

Un participant peut être malveillant. Il peut :

1. envoyer du code conçu pour épuiser le CPU ou la mémoire du vérificateur ;
2. annoncer un faux score ;
3. inonder le hub de capsules sans valeur ;
4. modifier une capsule en transit (métadonnées, lignée) ;
5. s'attribuer le travail d'un autre ;
6. créer de nombreuses fausses identités.

Hors périmètre de la v0 : confidentialité des capsules (tout est public), résistance à un
hub malveillant (le hub est de confiance).

## 4. Architecture retenue : hub et workers

Modèle en îles avec migration, à la manière de BOINC. Pas de pair-à-pair en v0 : la
traversée de NAT et la résistance aux fausses identités sans autorité centrale coûtent
des semaines et ne répondent pas à la question « la grille apporte-t-elle quelque chose ? ».

```
   Worker A (Qwen)        Worker B (DeepSeek)       Worker C (Gemma)
   LLM local + île        LLM local + île           LLM local + île
        |  ^                   |  ^                      |  ^
 submit |  | migrants   submit |  | migrants      submit |  | migrants
        v  |                   v  |                      v  |
   +-----------------------------------------------------------+
   |                         HUB                               |
   |  1. vérifie la forme et la signature                      |
   |  2. réévalue lui-même (sandbox + oracle)                  |
   |  3. insère dans l'archive MAP-Elites globale              |
   +-----------------------------------------------------------+
```

- **Worker** : fait tourner la boucle d'évolution actuelle (`EvolutionEngine`) sur une
  archive locale. À intervalle régulier, il envoie ses élites au hub et reçoit des
  migrants, qu'il réévalue avant de les insérer.
- **Hub** : détient l'archive globale. Il ne génère rien. Il réévalue tout ce qu'il
  reçoit. Le score retenu est toujours le sien.

### Règle centrale

**Personne ne transmet de score.** Seules les capsules circulent. Chaque récepteur, hub
ou worker, recalcule. Un faux score est donc impossible par construction (menace 2).

## 5. Protocole (HTTP + JSON, v0)

Toutes les requêtes portent `protocol_version`, `domain` et `dataset_hash`. Un worker
dont le `dataset_hash` diffère de celui du hub est refusé : cela détecte les versions
incompatibles de l'évaluateur.

| Route | Rôle |
|---|---|
| `GET /v0/experiment` | Paramètres : domaine, seed des jeux de données, hash attendus, limites |
| `POST /v0/capsules` | Soumettre un lot de capsules signées (taille de lot bornée) |
| `GET /v0/migrants?n=K&exclude=...` | Recevoir K élites de niches variées |
| `GET /v0/archive/summary` | Couverture, QD-score, meilleur score, nombre de contributeurs |

Réponse à une soumission, par capsule : `accepted`, `duplicate`, `rejected(raison)`.
Le hub ne renvoie pas le détail des scores sur les jeux de test, pour éviter qu'un worker
optimise contre eux.

## 6. Durcissements nécessaires avant toute ouverture

Classés par priorité. Les trois premiers sont des prérequis de la v0.

### 6.1 Hash canonique complet

Aujourd'hui, le hash couvre le nom, l'interface, le code, les parents, la génération, la
version d'interface et l'identifiant de greffe. Il ne couvre pas `metadata`,
`policy_kind`, `validation`, `dependencies`. Un intermédiaire peut donc les modifier
sans changer l'identifiant (menace 4).

- Définir un **hash de contenu v2** sur une sérialisation canonique des seuls champs qui
  définissent le comportement : `logic_code` normalisé (AST ré-imprimé), `interface_version`,
  `policy_kind`.
- Séparer ce qui est **identité** (le code) de ce qui est **provenance** (parents,
  génération, auteur, modèle). La provenance est signée, pas hachée dans l'identifiant.
  Deux nœuds qui découvrent la même heuristique obtiennent ainsi le même identifiant.
- `created_at` et `validation` sortent de l'objet transmis : le premier n'est pas
  déterministe, le second est recalculé par le récepteur.
- Passer l'identifiant de 64 à 128 bits au moins.

### 6.2 Bornes sur les entrées

Avant tout parsing d'une capsule reçue :

- taille maximale de `logic_code` (par exemple 8 Ko) et des autres champs ;
- profondeur d'imbrication maximale, mesurée sur le texte avant d'appeler `syn`
  (`syn` est récursif et peut faire déborder la pile sur une expression très imbriquée) ;
- signature de fonction exactement canonique ;
- liste blanche des constructions AST, déjà en grande partie assurée par l'interpréteur ;
- budget de fuel global par capsule, pas seulement par appel (menace 1).

### 6.3 Déterminisme des flottants entre machines

L'interpréteur expose `powf`, `ln` et `exp`. Leur résultat dépend de la bibliothèque
mathématique de la plateforme : deux nœuds honnêtes peuvent diverger au dernier bit, donc
choisir un nœud différent, donc obtenir un score différent.

Options, de la plus simple à la plus sûre :

1. interdire `powf`, `ln`, `exp` dans les capsules de grille (garder `powi`, `sqrt`,
   `abs`, `min`, `max`, `floor`, `ceil`, qui sont exacts en IEEE 754) ;
2. les implémenter en logiciel avec le crate `libm`, identique partout ;
3. passer l'interpréteur en virgule fixe.

Recommandation v0 : option 2. À valider par un test croisé Windows / Linux sur un lot de
capsules.

### 6.4 Identité et signature

- Une paire de clés Ed25519 par worker, générée au premier lancement.
- Chaque soumission est signée. Le hub associe contributions et quotas à la clé publique
  (menaces 3 et 5).
- Contre les fausses identités (menace 6) en v0 : inscription par invitation ou quota
  faible par clé nouvelle. Pas de preuve de travail.

### 6.5 Anti-inondation

- Quota de soumissions par clé et par heure.
- Déduplication par identifiant avant toute évaluation.
- Rejet rapide : le hub évalue d'abord sur quelques instances sondes et n'engage
  l'évaluation complète que si la capsule n'est pas dominée.

## 7. Fusion des archives

- L'ensemble des capsules est une union sans conflit.
- L'archive MAP-Elites garde le meilleur par niche. La fusion est déterministe à deux
  conditions : scores identiques partout (§6.3) et **ordre total** sur les candidats.
  Il faut donc un départage final par identifiant de capsule quand tous les critères
  sont égaux.
- Les compteurs `times_sampled` et `times_replaced` restent locaux à chaque nœud.

## 8. Séparation entraînement / test dans la grille

Si les workers connaissaient les jeux de test, ils pourraient optimiser dessus. Règle :

- les workers reçoivent la seed du jeu d'**entraînement** uniquement ;
- le hub seul évalue sur validation et hors distribution, et ne publie que des agrégats.

Cela demande que `SchedulingOracle` puisse être construit sans ses jeux de test.

## 9. Étapes de réalisation

Chaque étape est utile seule et testable sans la suivante.

| Étape | Contenu | Réseau ? | Question à laquelle elle répond |
|---|---|---|---|
| 0 | `nomad verify <capsule.json>` : bornes, hash v2, réévaluation | Non | Peut-on accepter une capsule étrangère en sécurité ? |
| 1 | `nomad export` / `nomad import` : migration entre deux runs par fichiers | Non | La migration entre îles améliore-t-elle le résultat ? |
| 2 | Test de déterminisme croisé Windows / Linux, passage à `libm` | Non | Deux machines calculent-elles le même score ? |
| 3 | Crate `nomad-grid` : `nomad serve` et `nomad worker` en LAN | Oui | Le protocole fonctionne-t-il ? |
| 4 | Signatures, quotas, hub exposé sur Internet par invitation | Oui | Tient-il face à des inconnus ? |
| 5 | Fédération de hubs, puis gossip | Oui | Peut-on se passer d'un hub unique ? |

L'étape 1 est le test décisif et ne coûte presque rien : deux runs avec des modèles
différents, on échange les élites à mi-parcours, on compare à deux runs isolés de même
budget. Si la migration n'apporte rien là, la grille n'apportera rien non plus.

## 10. Organisation du code

Tout le code de grille vit dans un nouveau crate, `crates/nomad-grid`, qui dépend des
crates existants sans les modifier, à trois exceptions prévues :

- `nomad-core` : hash de contenu v2 (§6.1), à côté de l'actuel pour ne pas invalider les
  rapports archivés ;
- `nomad-sandbox` : fonctions mathématiques déterministes (§6.3) ;
- `nomad-orchestrator` : départage final par identifiant (§7) et point d'entrée pour
  injecter des migrants dans un run en cours.

Dépendances envisagées : `tiny_http` côté hub (synchrone, cohérent avec `ureq` déjà
utilisé, pas besoin de `tokio`), `ed25519-dalek` pour les signatures, `libm`.

## 11. Questions ouvertes

1. Un seul problème par hub, ou plusieurs expériences en parallèle ?
2. Le hub doit-il aussi exécuter l'anti-dead-code, ou seulement les workers ?
3. Comment créditer un contributeur : capsules acceptées, niches conquises, descendance ?
4. Faut-il publier l'archive complète, ou seulement les élites ?
