"""Depouille le protocole docs/PROTOCOLE_GLM_LOCAL.md.

Deux bras d'evolution sur glm-5.3-flash, meme executable : `temoin` (groupe D seul) et
`local` (groupe D + croisements AST locaux + elagage du champion).

Usage : python scripts/analysis/glm_local.py [controle] [--seeds24]
  controle  : fait tous les calculs mais n'affiche que des controles de coherence, aucun
              score (pour verifier le script avant le depouillement).
  --seeds24 : depouillement de l'extension prevue (24 seeds, seuil 0,025).

Le champion apres k appels est la derniere proposition marquee is_new_best dont le nombre
d'appels deja consommes (llm_calls_consumed) est au plus k : les operateurs locaux, gratuits,
comptent au palier de l'appel qui les precede.
"""
import glob, json, os, re, statistics, subprocess, sys

EXE = os.path.abspath("target/frozen/nomad-local-ops.exe")
OUT = "results_glm_local"
ARMS = ["temoin", "local"]
N_SEEDS = 24 if "--seeds24" in sys.argv else 12
SEEDS = [9600 + 17 * r for r in range(N_SEEDS)]
ALPHA = 0.025 if N_SEEDS == 24 else 0.05
BUDGET = 344
PALIERS = [85, 170, 344]
PRICE_IN, PRICE_OUT = 0.0399, 0.0784  # z-ai/glm-5.3-flash chez Relace (OpenRouter)
INVALID = -100.0
CONTROLE = "controle" in sys.argv
LLM_KINDS = ("fresh", "mutation")
med = statistics.median


def refs(seed):
    """{jeu: (bacs de Best-Fit, borne L1)} pour la seed."""
    env = dict(os.environ, NOMAD_BP_PROFILE="weibull", NOMAD_FUEL_LIMIT="400000")
    out = subprocess.run([EXE, "eval", "--seed", str(seed), "--fuel", "400000", "--code",
                          "scripts/analysis/heuristics/best_fit.rs"],
                         capture_output=True, text=True, env=env).stdout
    found = re.findall(r"(\S+)\s*: valid=true \d+ bins \(excess: \+\d+\), Best-Fit: (\d+) \(excess: \+(\d+)\)", out)
    r = {lab: (int(b), int(b) - int(e)) for lab, b, e in found}
    assert "In-Dist" in r and "Valid" in r, (seed, out)
    return r


def eff(bins, ref):
    bf, l1 = ref
    return 100 * (bf - bins) / (bf - l1)


def champion_at(report, k):
    last = None
    for p in report["proposals"]:
        if p["llm_calls_consumed"] > k:
            break
        if p["is_new_best"]:
            last = p
    return last


def effects(p, ref):
    if p is None:
        return INVALID, INVALID
    v = eff(p["validation_bins"], ref["Valid"]) if p["validation_is_valid"] else INVALID
    t = eff(p["total_bins"], ref["In-Dist"]) if p["is_valid"] else INVALID
    return v, t


def usage(arm, seed):
    calls = prompt = completion = 0
    for path in glob.glob(f"{OUT}/{arm}/usage_seed_{seed}_essai*.jsonl"):
        for line in open(path, encoding="utf-8"):
            if line.strip():
                row = json.loads(line)
                calls += 1
                prompt += row.get("prompt_tokens") or 0
                completion += row.get("completion_tokens") or 0
    return calls, prompt, completion


def cost(prompt, completion):
    return prompt * PRICE_IN / 1e6 + completion * PRICE_OUT / 1e6


def wilcoxon_exact(a, b):
    """Wilcoxon signe apparie, bilateral, exact (rangs moyens en cas d'ex aequo)."""
    diffs = sorted((x - y for x, y in zip(a, b) if x != y), key=abs)
    n = len(diffs)
    if n == 0:
        return 0.0, 0, 1.0
    ranks2 = [0] * n
    i = 0
    while i < n:
        j = i
        while j + 1 < n and abs(diffs[j + 1]) == abs(diffs[i]):
            j += 1
        for k in range(i, j + 1):
            ranks2[k] = (i + 1) + (j + 1)
        i = j + 1
    total = sum(ranks2)
    w_plus = sum(r for r, d in zip(ranks2, diffs) if d > 0)
    counts = [0] * (total + 1)
    counts[0] = 1
    for r in ranks2:
        for s in range(total, r - 1, -1):
            counts[s] += counts[s - r]
    w_min = min(w_plus, total - w_plus)
    extreme = sum(c for s, c in enumerate(counts) if min(s, total - s) <= w_min)
    return w_plus / 2, n, extreme / (1 << n)


# ---------------------------------------------------------------- chargement
missing = [f"{a}/{s}" for a in ARMS for s in SEEDS
           if not os.path.exists(f"{OUT}/{a}/run_seed_{s}_{a}.json")]
if missing:
    sys.exit(f"Rapports manquants : {missing}")

R = {s: refs(s) for s in SEEDS}
reports = {a: {s: json.load(open(f"{OUT}/{a}/run_seed_{s}_{a}.json", encoding="utf-8"))
               for s in SEEDS} for a in ARMS}
curve = {a: {s: {k: effects(champion_at(reports[a][s], k), R[s]) for k in PALIERS}
             for s in SEEDS} for a in ARMS}

# ---------------------------------------------------------------- controles
problems = []
for a in ARMS:
    for s in SEEDS:
        rep = reports[a][s]
        props = rep["proposals"]
        llm = [p for p in props if p["kind"] in LLM_KINDS]
        if len(llm) != BUDGET:
            problems.append(f"{a}/{s} : {len(llm)} propositions du modele au lieu de {BUDGET}")
        if rep["budget_consumed"]["llm_calls_consumed"] != BUDGET:
            problems.append(f"{a}/{s} : budget {rep['budget_consumed']['llm_calls_consumed']}")
        calls, _, _ = usage(a, s)
        if calls < BUDGET:
            problems.append(f"{a}/{s} : {calls} appels journalises")
        last = 0
        for p in props:
            if p["llm_calls_consumed"] < last:
                problems.append(f"{a}/{s} : journal non ordonne")
                break
            last = p["llm_calls_consumed"]
        local = [p for p in props if p["kind"] not in LLM_KINDS]
        if a == "temoin" and local:
            problems.append(f"{a}/{s} : {len(local)} propositions locales dans le temoin")
        if a == "local" and not any(p["kind"] == "crossover" for p in props):
            problems.append(f"{a}/{s} : aucun croisement local")
        for p in props:
            if p["is_new_best"] and p.get("validation_bins") is None:
                problems.append(f"{a}/{s} : champion {p['index']} sans note de validation")
                break
print(f"Controles : {len(SEEDS)} seeds x {len(ARMS)} bras, {len(problems)} probleme(s)")
for p in problems:
    print("  ", p)
if CONTROLE:
    print("Mode controle : aucun score affiche.")
    sys.exit(0)

# ---------------------------------------------------------------- tableaux
for idx, label in [(0, "Validation"), (1, "Entrainement")]:
    print(f"\n== {label} : effet (part de l'exces de Best-Fit supprimee, %) par palier")
    print("   seed  | " + " | ".join(f"loc {k:4d} / tem {k:4d}" for k in PALIERS))
    for s in SEEDS:
        cells = [f"{curve['local'][s][k][idx]:8.1f} / {curve['temoin'][s][k][idx]:8.1f}" for k in PALIERS]
        print(f"   {s}  | " + " | ".join(cells))
    for a in ARMS:
        print(f"   {a} :")
        for k in PALIERS:
            v = [curve[a][s][k][idx] for s in SEEDS]
            print(f"      {k:4d} appels : mediane {med(v):6.1f}  >=10% : {sum(x >= 10 for x in v):2d}/{len(v)}"
                  f"  >=30% : {sum(x >= 30 for x in v):2d}/{len(v)}  invalides : {sum(x == INVALID for x in v)}")

print("\n== Ecart local - temoin sur la validation, par palier")
for k in PALIERS:
    d = [curve["local"][s][k][0] - curve["temoin"][s][k][0] for s in SEEDS]
    print(f"   {k:4d} appels : ecart median {med(d):6.1f} points, seeds gagnees par le bras local "
          f"{sum(x > 0 for x in d):2d}, perdues {sum(x < 0 for x in d):2d}, egalites {sum(x == 0 for x in d):2d}")

print("\n== Exces sur la borne L1 (validation, % de L1, descriptif ajoute le 16/11 apres depouillement)")
for k in PALIERS:
    cells = []
    for a in ARMS:
        v = []
        for s in SEEDS:
            p = champion_at(reports[a][s], k)
            if p is not None and p["validation_is_valid"]:
                l1 = R[s]["Valid"][1]
                v.append(100 * (p["validation_bins"] - l1) / l1)
        cells.append(f"{a} {med(v):5.2f} %")
    print(f"   {k:4d} appels : " + ", ".join(cells))
bf_exc = [100 * (R[s]["Valid"][0] - R[s]["Valid"][1]) / R[s]["Valid"][1] for s in SEEDS]
print(f"   Best-Fit : {med(bf_exc):5.2f} %")

print("\n== Origine du champion final (type de la proposition)")
for a in ARMS:
    kinds = {}
    for s in SEEDS:
        p = champion_at(reports[a][s], BUDGET)
        kinds[p["kind"]] = kinds.get(p["kind"], 0) + 1
    print(f"   {a} : {kinds}")
print("   champions successifs du bras local, par type :",
      {k: sum(1 for s in SEEDS for p in reports['local'][s]['proposals'] if p['is_new_best'] and p['kind'] == k)
       for k in ("fresh", "mutation", "crossover", "prune")})

print("\n== Operateurs locaux (bras local, totaux sur les seeds)")
tot = {}
for s in SEEDS:
    for key, v in reports["local"][s].get("local_ops", {}).items():
        tot[key] = tot.get(key, 0) + v
print("  ", tot)

print("\n== Taille du code du champion final (caracteres, mediane)")
for a in ARMS:
    print(f"   {a} : {med(len(champion_at(reports[a][s], BUDGET)['logic_code']) for s in SEEDS):.0f}")

print("\n== Tokens et cout")
per_call = {}
for a in ARMS:
    calls = prompt = completion = 0
    for s in SEEDS:
        c, pi, co = usage(a, s)
        calls, prompt, completion = calls + c, prompt + pi, completion + co
    per_call[a] = cost(prompt, completion) / calls
    print(f"   {a} : requetes {calls}, tokens entree {prompt}, sortie {completion}, "
          f"cout estime {cost(prompt, completion):.2f} $, {per_call[a]:.6f} $ par appel")
print(f"   rapport de cout par appel local / temoin = {per_call['local'] / per_call['temoin']:.4f}")

print(f"\n== H (appels egaux) : bras local contre temoin a {BUDGET} appels, {len(SEEDS)} seeds")
a = [curve["local"][s][BUDGET][0] for s in SEEDS]
b = [curve["temoin"][s][BUDGET][0] for s in SEEDS]
d = [x - y for x, y in zip(a, b)]
wins, losses = sum(x > 0 for x in d), sum(x < 0 for x in d)
w, n, p = wilcoxon_exact(a, b)
print(f"   en faveur {wins}, en defaveur {losses}, egalites {len(d) - wins - losses}; difference mediane {med(d):.1f} points")
print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w}, n = {n}, p = {p:.4f}  (seuil {ALPHA})")
if p < ALPHA and wins > losses:
    print("   H ETABLIE")
elif p < ALPHA and losses > wins:
    print("   EFFET SIGNIFICATIF EN SENS CONTRAIRE")
else:
    print("   H non etablie")
    if N_SEEDS == 12 and p < 0.20 and wins > losses:
        print("   Condition d'extension remplie (p < 0,20, plus de seeds gagnees que perdues) : 12 seeds de plus")
