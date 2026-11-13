"""Depouille les bras du protocole docs/PROTOCOLE_MINIMAX_TIRAGE_EVOLUTION.md.

Usage : python scripts/analysis/minimax_tirage_evolution.py
Le bras evolution est ignore tant que ses 12 rapports ne sont pas presents.
"""
import glob, json, os, re, statistics, subprocess

EXE = os.path.abspath("target/frozen/nomad-minimax-v1.exe")
OUT = "results_minimax_v1"
SEEDS = list(range(4000, 4188, 17))
SETS = [("Entrainement", "best_in_distribution_score", "In-Dist"),
        ("Validation", "best_validation_score", "Valid"),
        ("OOD-1", "best_ood1_score", "OOD-1"),
        ("OOD-2", "best_ood2_score", "OOD-2")]


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


arms = [a for a in ("tirage", "evolution")
        if all(os.path.exists(f"{OUT}/{a}/run_seed_{s}_{a}.json") for s in SEEDS)]
refs = {s: best_fit(s) for s in SEEDS}
res = {a: {} for a in arms}
for a in arms:
    for s in SEEDS:
        run = json.load(open(f"{OUT}/{a}/run_seed_{s}_{a}.json", encoding="utf-8"))
        res[a][s] = {lab: effect(run, key, lab, refs[s]) for _, key, lab in SETS}
        res[a][s]["calls"] = run["budget_consumed"]["llm_calls_consumed"]
        res[a][s]["name"] = run["best_capsule"]["name"]

fmt = lambda v: "   n/a" if v is None else f"{v:6.1f}"
for name, _, lab in SETS:
    print(f"== {name} (exces de Best-Fit supprime, %)  seed | " + " | ".join(arms))
    for s in SEEDS:
        print(f"  {s} | " + " | ".join(fmt(res[a][s][lab]) for a in arms))
    for a in arms:
        v = [res[a][s][lab] for s in SEEDS if res[a][s][lab] is not None]
        print(f"   {a}: mediane {statistics.median(v):.1f}  succes>=10%: {sum(x >= 10 for x in v)}/12"
              f"  invalides: {12 - len(v)}")

for a in arms:
    tot = [usage(a, s) for s in SEEDS]
    p, c = sum(t[1] for t in tot), sum(t[2] for t in tot)
    print(f"-- {a}: appels du moteur {[res[a][s]['calls'] for s in SEEDS]}")
    print(f"   requetes journalisees {sum(t[0] for t in tot)}, tokens entree {p}, sortie {c},"
          f" cout estime {p * 0.30 / 1e6 + c * 1.20 / 1e6:.2f} $")
    print("   champions:", {s: res[a][s]["name"] for s in SEEDS})

if len(arms) == 2:
    pairs = [(res["evolution"][s]["Valid"], res["tirage"][s]["Valid"]) for s in SEEDS]
    ok = [(e, t) for e, t in pairs if e is not None and t is not None]
    print(f"== Validation : evolution meilleure sur {sum(e > t for e, t in ok)} seeds,"
          f" tirage sur {sum(t > e for e, t in ok)}, sur {len(ok)} paires valides")
    # Wilcoxon signe apparie, bilateral, exact par enumeration des 2^n signes.
    diffs = sorted((e - t for e, t in ok if e != t), key=abs)
    n = len(diffs)
    w_plus = sum(i + 1 for i, d in enumerate(diffs) if d > 0)
    w_min = min(w_plus, n * (n + 1) // 2 - w_plus)
    count = sum(1 for mask in range(1 << n)
                if min(w := sum(i + 1 for i in range(n) if mask >> i & 1), n * (n + 1) // 2 - w) <= w_min)
    print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w_plus}, n = {n}, p = {count / (1 << n):.3f}")
    for a in arms:
        v = [res[a][s]["Valid"] for s in SEEDS if res[a][s]["Valid"] is not None]
        print(f"   {a}: seeds >= 30 % sur validation : {sum(x >= 30 for x in v)}, maximum {max(v):.1f}")
