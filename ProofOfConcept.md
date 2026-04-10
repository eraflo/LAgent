# L-Agent : un langage de programmation système pour l'ère des LLM

**Spécification conceptuelle – Version 0.1 – Avril 2026**

## 1. Introduction

### 1.1. Contexte et analogie fondatrice

Le langage C a été conçu pour faire le pont entre la logique algorithmique humaine et la réalité physique du matériel : registres, mémoire, interruptions. Il offre un contrôle fin, déterministe et prévisible sur les ressources.

En 2026, le "matériel" ne se limite plus au CPU ou au GPU. Il inclut désormais :

- La **fenêtre de contexte** des grands modèles de langage (LLM), ressource limitée et coûteuse.
- Le **moteur d'inférence**, capable d'évaluer des probabilités et d'effectuer des raisonnements.
- Les **flux d'interaction** entre l'utilisateur et l'agent, souvent asynchrones et sujets à des interruptions.

**L-Agent** est un nouveau langage de programmation système conçu pour être à cette nouvelle couche matérielle ce que C est au silicium. Il offre un contrôle explicite, performant et fiable sur les primitives de l'IA agentique, via une syntaxe dédiée et une chaîne de compilation complète.

### 1.2. Un langage à part entière

Contrairement aux bibliothèques ou frameworks existants, L-Agent se définit par :

- Une **syntaxe autonome** et des fichiers source portant l'extension `.la`.
- Un **compilateur** écrit en Rust, traduisant le code source en bytecode exécutable.
- Une **machine virtuelle (VM)** portable, exécutant le bytecode et interagissant avec les backends d'inférence (locaux ou distants).
- Un **écosystème d'outillage** (formateur, débogueur, LSP) prévu pour une intégration complète dans les environnements de développement.

### 1.3. Positionnement par rapport à l'existant

Des projets comme **Wenli (文理)** , **NERD**, **SPL**, ou **Aragog** montrent que la programmation des LLM évolue vers plus de structure et d'abstraction. Cependant, ces solutions restent :

- Déclaratives et de haut niveau (SPL, Wenli).
- Des formats intermédiaires optimisés pour la consommation machine (NERD).
- Des orchestrateurs plutôt que des langages système (Aragog).

**Aucun projet ne propose un langage compilé de bas niveau offrant un contrôle explicite sur l'allocation de contexte, le routage probabiliste du flux de contrôle ou les interruptions déterministes.** C'est cette niche que L-Agent entend occuper en tant que **langage de programmation indépendant**.

---

## 2. Présentation du langage

### 2.1. Exemple de code source (`exemple.la`)

```la
// Définition d'un type sémantique
type Emotion = semantic("joie", "colère", "tristesse", "neutre");

// Définition d'un kernel de raisonnement
kernel AnalyserMessage(texte: str) -> Emotion {
    observe(texte);
    reason("Déterminer l'émotion dominante");
    let emotion: Emotion = infer(texte);
    verify(emotion != "neutre");
    return emotion;
}

// Point d'entrée du programme
fn main() {
    let ctx = ctx_alloc(4096);
    ctx_append(ctx, "Je suis très mécontent de ce service !");
    
    // Branchement probabiliste basé sur l'intention
    branch intent {
        case "angry" (confidence > 0.7) => {
            println("Gestion de crise activée");
            kernel_handle_crisis(ctx);
        }
        case "help" (confidence > 0.4) => {
            println("Support standard");
            kernel_provide_support(ctx);
        }
        default => {
            println("Redirection vers un opérateur humain");
        }
    }
    
    ctx_free(ctx);
}
```

### 2.2. Extension des fichiers et structure d'un projet

- **Fichiers sources :** `*.la`
- **Fichier de projet (optionnel) :** `lagent.toml` (dépendances, modèles locaux, configuration de compilation)
- **Sortie du compilateur :** bytecode au format `.lbc` (L-Agent Bytecode) ou transpilation vers un autre langage.

---

## 3. Architecture du compilateur

Le compilateur L-Agent est écrit en Rust. Il suit une architecture classique de compilateur moderne.

### 3.1. Vue d'ensemble

Source `.la` → [Lexer] → Tokens → [Parser] → AST → [Analyse sémantique] → AST typé → [Générateur de bytecode] → Bytecode `.lbc`

### 3.2. Lexer

- **Rôle :** découper le texte source en une séquence de tokens (mots-clés, identifiants, littéraux, symboles).
- **Implémentation :** bibliothèque Rust `logos` pour une définition déclarative et performante.

**Extrait de la définition des tokens :**
```rust
#[derive(Logos)]
enum Token {
    #[token("fn")] Fn,
    #[token("kernel")] Kernel,
    #[token("branch")] Branch,
    #[token("case")] Case,
    #[token("semantic")] Semantic,
    #[token("ctx_alloc")] CtxAlloc,
    // ...
}
```

### 3.3. Parser

- **Rôle :** analyser la séquence de tokens et produire un Arbre Syntaxique Abstrait (AST) conforme à la grammaire du langage.
- **Implémentation :** `chumsky`, une bibliothèque de parser combinators expressive et robuste.

### 3.4. Analyse sémantique

- **Rôle :**
    - Résolution des noms (fonctions, variables, types).
    - Vérification des types (types primitifs et types sémantiques).
    - Validation des contraintes spécifiques (ex : les labels de case doivent correspondre à des concepts du type sémantique associé).
    - Construction de la table des symboles.
- **Implémentation :** parcours de l'AST avec un contexte mutable.

### 3.5. Génération de bytecode

- **Rôle :** traduire l'AST typé en une séquence d'instructions de bas niveau (bytecode) exécutable par la VM.
- **Implémentation :** émission d'instructions dans un buffer, résolution des adresses de saut, optimisation simple (optionnel).

**Exemple d'instructions de bytecode :**
```rust
enum OpCode {
    CtxAlloc(u32),                 // alloue un segment de contexte
    CtxFree(u8),                   // libère un segment
    CtxAppend(u8, u8),             // ajoute une chaîne au segment
    Branch { cases: Vec<(String, f32, u16)>, default: u16 },
    CallKernel(u16),               // appelle un kernel
    LocalInfer(u8, u8, u8),        // inférence locale
    // ...
}
```

---

## 4. Machine Virtuelle (VM) et Runtime

La VM est le moteur d'exécution du bytecode. Elle est écrite en Rust et embarque les capacités d'interaction avec les LLM.

### 4.1. Responsabilités

- Charger et exécuter le bytecode.
- Gérer la mémoire du Token Heap (allocation/libération de contexte).
- Orchestrer les appels aux modèles de langage (locaux ou distants).
- Fournir des fonctions natives (entrées/sorties, manipulation de chaînes, etc.).

### 4.2. Gestion du contexte : le Token Heap

Implémentation d'un gestionnaire de segments de contexte analogue à un tas mémoire :

| Concept C | Équivalent L-Agent |
| :--- | :--- |
| `malloc(size)` | `ctx_alloc(tokens: usize) -> CtxSegment` |
| `free(ptr)` | `ctx_free(segment: CtxSegment)` |
| `realloc` | `ctx_resize(segment: CtxSegment, new_tokens: usize)` |
| Fuite mémoire | Épuisement du contexte → `CTX_OVERFLOW` |

**Primitives avancées :**

- `ctx_compress(segment: CtxSegment, model: &CompressorModel)` : résume automatiquement le contenu pour libérer de l'espace tout en préservant la substance sémantique.
- `ctx_share(segment: CtxSegment, agent_id: AgentId)` : partage un segment entre plusieurs agents (mémoire partagée).

### 4.3. Backends d'inférence

La VM supporte plusieurs backends, sélectionnables à la compilation ou à l'exécution :

| Backend | Description | Implémentation Rust |
| :--- | :--- | :--- |
| **Local (GGUF)** | Modèles quantifiés exécutés localement | `llama-cpp-rs` ou `candle` |
| **Local (ONNX)** | Modèles au format ONNX | `ort` (ONNX Runtime) |
| **Distant (API)** | Appels aux API cloud (OpenAI, Anthropic) | `reqwest` + feature flag remote |
| **Simulé** | Pour les tests, retourne des résultats déterministes | Implémentation native |

### 4.4. Gestion des interruptions (Safe Interaction Points)

La VM expose un mécanisme de checkpointing pour les blocs `interruptible`. Lorsqu'une interruption est signalée (par exemple via un signal système ou une socket), l'état de la VM (contexte, pile d'appels) est sauvegardé et peut être restauré après traitement de l'interruption.

---

## 5. Grands axes syntaxiques et sémantiques

### 5.1. Types de données sémantiques

**Syntaxe :**
```la
type NomType = semantic("concept1", "concept2", ...);
```

**Sémantique :**
- Le compilateur vérifie que toute valeur assignée à une variable de ce type est sémantiquement proche d'au moins un des concepts listés.
- La validation repose sur une mesure de distance cosinus dans l'espace d'embedding du modèle cible.

### 5.2. Branchement probabiliste (branch)

**Syntaxe :**
```la
branch <variable> {
    case "label1" (confidence > seuil1) => { ... }
    case "label2" (confidence > seuil2) => { ... }
    default => { ... }
}
```

**Sémantique :**
- Le runtime évalue les probabilités des labels par inférence contrainte (logits).
- La première branche dont la confiance dépasse le seuil est exécutée.
- Si aucune, la branche `default` est prise.

**Extensions :**
- `branch multi` pour exécuter plusieurs branches en parallèle (fork agentique).
- `branch fallback` avec un modèle moins coûteux si le modèle principal n'atteint aucun seuil.

### 5.3. Kernels de raisonnement (kernel)

**Syntaxe :**
```la
kernel NomKernel(params) -> TypeRetour {
    observe(expr);
    reason("instruction textuelle");
    act(expr);
    verify(condition);
    // ...
}
```

**Sémantique :**
- Chaque étape est tracée et peut être ré-exécutée en cas d'échec de `verify`.
- Les kernels sont des unités réutilisables et testables.

### 5.4. Primitives d'exécution locale

Le langage expose des fonctions built-in pour la gestion explicite des modèles locaux :
- `local_model_load(path: str, device: str) -> ModelHandle`
- `local_model_infer(handle: ModelHandle, prompt: str) -> InferenceResult`
- `local_model_unload(handle: ModelHandle)`
- `local_model_list() -> [ModelInfo]`

**Politiques de sécurité pour les modèles locaux :**
- `--sandbox-fs` : restreint l'accès du modèle aux fichiers.
- `--max-tokens-per-call` : limite le nombre de tokens générés par appel.
- `--timeout-ms` : interrompt l'inférence si elle dépasse un certain temps.

### 5.5. Directives de compilation

Le compilateur accepte des drapeaux influençant la stratégie de génération de code :
- `-O cost` : privilégie les modèles les moins coûteux.
- `-O precision` : privilégie les modèles les plus performants.
- `-O latency` : optimise pour le temps de réponse.
- `-O local` : force l'usage exclusif de modèles locaux.

---

## 6. Lacunes identifiées et axes d'enrichissement

### 6.1. Déterminisme et reproductibilité
Problème : Les LLM sont intrinsèquement non-déterministes.
Pistes : Mode `--deterministic` (temperature=0), primitives de logging sémantique, et rejeu (replay).

### 6.2. Interopérabilité
Problème : Besoin d'appeler des APIs classiques et d'être appelé depuis Python/Rust/JS.
Pistes : FFI Agentique, bindings automatiques (PyO3), et support de NERD comme cible intermédiaire.

### 6.3. Gestion des erreurs et dégradation gracieuse
Problème : Modèle indisponible ou contexte saturé.
Pistes : Hiérarchie de fallbacks, mécanisme de backpressure, et mode dégradé.

### 6.4. Sécurité et sandboxing
Problème : Risques d'injection via le code influencé par un LLM.
Pistes : Sandboxing strict, politiques `allow_network = false`, validation sémantique.

### 6.5. Observabilité et debugging
Problème : Déboguer un programme probabiliste est complexe.
Pistes : Traces explicables, visualisation de graphes, mode pas-à-pas.

### 6.6. Évolutivité des modèles
Problème : L'obsolescence des types sémantiques face aux nouveaux modèles.
Pistes : Types descriptifs (textuels) ré-encodés à la volée, versioning des types.

### 6.7. Gestion des ressources locales
Problème : Contraintes fortes sur la mémoire locale.
Pistes : Quotas mémoire, swap de modèles GPU, partage de contexte (tokenizer commun).

---

## 7. Chaîne d'outillage

À terme, l'écosystème L-Agent comprendra :
- `lagent` : le compilateur et gestionnaire de projets (build, run, fmt).
- `lagent-lsp` : serveur de langage (auto-complétion, vérification).
- `lagent-dbg` : débogueur interactif avec inspection du contexte.
- `lagent-fmt` : formateur de code.

---

## 8. Feuille de route de développement

### Phase 1 – Preuve de concept (3 mois)
- Lexer et parser pour un sous-ensemble du langage.
- Génération d'un bytecode minimal.
- VM capable d'exécuter `ctx_alloc`, `ctx_free`.
- Premier appel local via Candle.

### Phase 2 – Langage minimal viable (6 mois)
- Support du `branch` probabiliste.
- Types sémantiques basiques.
- Compilation croisée simple.
- CLI fonctionnelle (`lagent run`).

### Phase 3 – Fonctionnalités avancées (9 mois)
- Implémentation complète des kernels (`verify`).
- Safe Interaction Points (`interruptible`).
- Gestion avancée des ressources locales (swap, quotas).

### Phase 4 – Maturité et outillage (12+ mois)
- Serveur LSP et débogueur visuel.
- Gestionnaire de paquets.
- Cas d'usage réels et documentation.

---

## 9. Conclusion

L-Agent est une proposition ambitieuse : un nouveau langage de programmation système conçu spécifiquement pour l'ère des modèles de langage. En offrant une syntaxe dédiée, un contrôle explicite sur le contexte et les flux probabilistes, et une exécution performante via une VM intégrée, il vise à rendre la programmation des agents IA aussi fiable, prévisible et efficace que la programmation système traditionnelle.

Le compilateur, écrit en Rust, bénéficie de la robustesse et des performances de ce langage. La VM, également en Rust, permet une intégration fine avec les moteurs d'inférence locaux et distants.

Concevoir un langage complet est un chemin exigeant, mais les premières briques sont posées. L'approche incrémentale proposée permet d'obtenir rapidement un prototype fonctionnel, puis de l'enrichir progressivement pour en faire un outil de production.