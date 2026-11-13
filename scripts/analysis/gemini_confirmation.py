"""Depouille le protocole docs/PROTOCOLE_GEMINI_CONFIRMATION.md.

Usage : python scripts/analysis/gemini_confirmation.py
Un bras n'est depouille que si ses 24 rapports sont presents.
"""
import glob, json, os, re, statistics, subprocess

EXE = os.path.abspath("target/frozen/nomad-gemini-v1.exe")
OUT = "results_gemini_v1"
SEEDS = [6000 + 17 * r for r in range(24)]
ARMS = ["tirage85", "evolution", "tirage170"]
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


arms = [a for a in ARMS if all(os.path.exists(f"{OUT}/{a}/run_seed_{s}_{a}.json") for s in SEEDS)]
print("Bras complets :", arms)
refs = {s: best_fit(s) for s in SEEDS}
res = {a: {} for a in arms}
for a in arms:
    for s in SEEDS:
        run = json.load(open(f"{OUT}/{a}/run_seed_{s}_{a}.json", encoding="utf-8"))
        res[a][s] = {lab: effect(run, key, lab, refs[s]) for _, key, lab in SETS}
        res[a][s]["calls"] = run["budget_consumed"]["llm_calls_consumed"]
        res[a][s]["name"] = run["best_capsule"]["name"]

fmt = lambda v: "invalide" if v is None else f"{v:8.1f}"
val = lambda v: INVALID if v is None else v
for name, _, lab in SETS:
    print(f"== {name} (exces de Best-Fit supprime, %)  seed | " + " | ".join(arms))
    for s in SEEDS:
        print(f"  {s} | " + " | ".join(fmt(res[a][s][lab]) for a in arms))
    for a in arms:
        v = [val(res[a][s][lab]) for s in SEEDS]
        invalid = sum(res[a][s][lab] is None for s in SEEDS)
        print(f"   {a:10}: mediane {statistics.median(v):6.1f}  succes>=10%: {sum(x >= 10 for x in v):2}/24"
              f"  succes>=30%: {sum(x >= 30 for x in v):2}/24  invalides: {invalid}")

print("== Appels et cout")
for a in arms:
    tot = [usage(a, s) for s in SEEDS]
    p, c = sum(t[1] for t in tot), sum(t[2] for t in tot)
    calls = sorted(set(res[a][s]["calls"] for s in SEEDS))
    print(f"   {a:10}: appels par run {calls}, requetes journalisees {sum(t[0] for t in tot)},"
          f" tokens entree {p}, sortie {c}, cout estime {p * PRICE_IN / 1e6 + c * PRICE_OUT / 1e6:.2f} $")
if "evolution" in arms:
    untouched = [s for s in SEEDS if "-mut-" not in res["evolution"][s]["name"]]
    print(f"   champions de l'evolution non modifies (propositions fraiches) : {len(untouched)}/24 {untouched}")

for label, other in (("H1 (appels egaux)", "tirage85"), ("H2 (depense egale)", "tirage170")):
    if "evolution" in arms and other in arms:
        e = [val(res["evolution"][s]["Valid"]) for s in SEEDS]
        t = [val(res[other][s]["Valid"]) for s in SEEDS]
        w, n, p = wilcoxon_exact(e, t)
        wins, losses = sum(x > y for x, y in zip(e, t)), sum(x < y for x, y in zip(e, t))
        print(f"== {label} : evolution contre {other} sur la validation")
        print(f"   seeds gagnees par l'evolution {wins}, perdues {losses}, egalites {24 - wins - losses}")
        print(f"   difference mediane {statistics.median(x - y for x, y in zip(e, t)):.1f} points")
        print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w}, n = {n}, p = {p:.4f}")
