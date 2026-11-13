"""Depouille le protocole docs/PROTOCOLE_GEMINI_ECHELLE.md.

Usage : python scripts/analysis/gemini_echelle.py [pilote]
Un bras n'est depouille que si ses 24 rapports sont presents.
Avec l'argument `pilote`, affiche seulement le cout et la duree de l'essai prealable.
"""
import glob, json, math, os, re, statistics, subprocess, sys

EXE = os.path.abspath("target/frozen/nomad-gemini-echelle.exe")
OUT = "results_gemini_echelle"
SEEDS = [7000 + 17 * r for r in range(24)]
ARMS = ["tirage340", "evolution340", "tirageDepense"]
# Protocole de confirmation (85 appels), pour la comparaison d'echelle H3.
OUT_85 = "results_gemini_v1"
SEEDS_85 = [6000 + 17 * r for r in range(24)]
SETS = [("Entrainement", "best_in_distribution_score", "In-Dist"),
        ("Validation", "best_validation_score", "Valid"),
        ("OOD-1", "best_ood1_score", "OOD-1"),
        ("OOD-2", "best_ood2_score", "OOD-2")]
PRICE_IN, PRICE_OUT = 0.25, 1.50  # dollars par million de tokens, gemini-3.1-flash-lite
INVALID = -100.0  # effet attribue a un champion invalide (protocole)


def best_fit(seed):
    env = dict(os.environ, NOMAD_BP_PROFILE="weibull", NOMAD_FUEL_LIMIT="400000")
    out = subprocess.run([EXE, "eval", "--seed", str(seed), "--fuel", "400000", "--code",
                          "scripts/analysis/heuristics/best_fit.rs"],
                         capture_output=True, text=True, env=env).stdout
    found = re.findall(r"(\S+)\s*: valid=true \d+ bins \(excess: \+\d+\), Best-Fit: (\d+) \(excess: \+(\d+)\)", out)
    return {lab: (int(b), int(b) - int(e)) for lab, b, e in found}


def effect(run, key, lab, ref):
    score = run[key]
    if not score["is_valid"]:
        return None
    bf, l1 = ref[lab]
    return 100 * (bf - score["total_bins"]) / (bf - l1)


def usage(out, arm, seed):
    calls = prompt = completion = 0
    for path in glob.glob(f"{out}/{arm}/usage_seed_{seed}_essai*.jsonl"):
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
    ranks2 = [0] * n  # rangs doubles, pour rester en entiers avec les ex aequo
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


def mann_whitney(x, y):
    """Mann-Whitney bilateral, approximation normale avec correction de continuite et
    d'ex aequo. Renvoie U de x et p."""
    pooled = sorted([(v, 0) for v in x] + [(v, 1) for v in y])
    n = len(pooled)
    ranks = [0.0] * n
    ties = 0.0
    i = 0
    while i < n:
        j = i
        while j + 1 < n and pooled[j + 1][0] == pooled[i][0]:
            j += 1
        for k in range(i, j + 1):
            ranks[k] = (i + j) / 2 + 1
        t = j - i + 1
        ties += t ** 3 - t
        i = j + 1
    n1, n2 = len(x), len(y)
    u1 = sum(r for r, (_, g) in zip(ranks, pooled) if g == 0) - n1 * (n1 + 1) / 2
    var = n1 * n2 / 12 * ((n + 1) - ties / (n * (n - 1)))
    z = (abs(u1 - n1 * n2 / 2) - 0.5) / math.sqrt(var)
    return u1, math.erfc(max(z, 0.0) / math.sqrt(2))


def load(out, arms, seeds):
    refs = {s: best_fit(s) for s in seeds}
    res = {a: {} for a in arms}
    for a in arms:
        for s in seeds:
            run = json.load(open(f"{out}/{a}/run_seed_{s}_{a}.json", encoding="utf-8"))
            res[a][s] = {lab: effect(run, key, lab, refs[s]) for _, key, lab in SETS}
            res[a][s]["calls"] = run["budget_consumed"]["llm_calls_consumed"]
            res[a][s]["name"] = run["best_capsule"]["name"]
    return res


val = lambda v: INVALID if v is None else v

if sys.argv[1:] == ["pilote"]:
    out, arm = "results_gemini_echelle_pilote", "evolution340"
    for path in sorted(glob.glob(f"{out}/{arm}/run_seed_*_{arm}.json")):
        seed = int(re.search(r"seed_(\d+)_", path).group(1))
        run = json.load(open(path, encoding="utf-8"))
        calls, p, c = usage(out, arm, seed)
        print(f"seed {seed}: appels {run['budget_consumed']['llm_calls_consumed']}, requetes {calls},"
              f" entree {p}, sortie {c}, cout {cost(p, c):.4f} $")
    sys.exit(0)

arms = [a for a in ARMS if all(os.path.exists(f"{OUT}/{a}/run_seed_{s}_{a}.json") for s in SEEDS)]
print("Bras complets :", arms)
res = load(OUT, arms, SEEDS)

fmt = lambda v: "invalide" if v is None else f"{v:8.1f}"
for name, _, lab in SETS:
    print(f"== {name} (exces de Best-Fit supprime, %)  seed | " + " | ".join(arms))
    for s in SEEDS:
        print(f"  {s} | " + " | ".join(fmt(res[a][s][lab]) for a in arms))
    for a in arms:
        v = [val(res[a][s][lab]) for s in SEEDS]
        invalid = sum(res[a][s][lab] is None for s in SEEDS)
        print(f"   {a:13}: mediane {statistics.median(v):6.1f}  succes>=10%: {sum(x >= 10 for x in v):2}/24"
              f"  succes>=30%: {sum(x >= 30 for x in v):2}/24  invalides: {invalid}")

print("== Appels et cout")
for a in arms:
    tot = [usage(OUT, a, s) for s in SEEDS]
    p, c = sum(t[1] for t in tot), sum(t[2] for t in tot)
    calls = sorted(set(res[a][s]["calls"] for s in SEEDS))
    print(f"   {a:13}: appels par run {calls}, requetes journalisees {sum(t[0] for t in tot)},"
          f" tokens entree {p}, sortie {c}, cout estime {cost(p, c):.2f} $")
if "evolution340" in arms:
    untouched = [s for s in SEEDS if "-mut-" not in res["evolution340"][s]["name"]]
    print(f"   champions de l'evolution non modifies (propositions fraiches) : {len(untouched)}/24 {untouched}")

for label, other in (("H1 (appels egaux)", "tirage340"), ("H2 (depense egale)", "tirageDepense")):
    if "evolution340" in arms and other in arms:
        e = [val(res["evolution340"][s]["Valid"]) for s in SEEDS]
        t = [val(res[other][s]["Valid"]) for s in SEEDS]
        w, n, p = wilcoxon_exact(e, t)
        wins, losses = sum(x > y for x, y in zip(e, t)), sum(x < y for x, y in zip(e, t))
        print(f"== {label} : evolution340 contre {other} sur la validation")
        print(f"   seeds gagnees par l'evolution {wins}, perdues {losses}, egalites {24 - wins - losses}")
        print(f"   difference mediane {statistics.median(x - y for x, y in zip(e, t)):.1f} points")
        print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w}, n = {n}, p = {p:.4f}")

if "evolution340" in arms and "tirage340" in arms:
    old = load(OUT_85, ["evolution", "tirage85"], SEEDS_85)
    d85 = [val(old["evolution"][s]["Valid"]) - val(old["tirage85"][s]["Valid"]) for s in SEEDS_85]
    d340 = [val(res["evolution340"][s]["Valid"]) - val(res["tirage340"][s]["Valid"]) for s in SEEDS]
    u, p = mann_whitney(d340, d85)
    print("== H3 (echelle) : ecart evolution - tirage a appels egaux, 340 appels contre 85")
    print(f"   ecart median a 85 appels {statistics.median(d85):.1f} points, a 340 appels {statistics.median(d340):.1f} points")
    print(f"   Mann-Whitney bilateral (approximation normale) : U = {u}, p = {p:.4f}")
