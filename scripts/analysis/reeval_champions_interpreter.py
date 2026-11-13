"""Reevalue les champions de toutes les campagnes avec deux executables et compare.

Sert a mesurer l'effet de la correction de l'interpreteur (voir
docs/CORRECTION_INTERPRETEUR.md) : l'ancien executable fige et le nouveau evaluent le meme
code, sur les memes jeux, avec le meme fuel.

Usage : python scripts/analysis/reeval_champions_interpreter.py <ancien.exe> <nouveau.exe> [sortie.json]
"""
import glob, json, os, re, statistics, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor

OLD, NEW = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
DUMP = sys.argv[3] if len(sys.argv) > 3 else None
CAMPAIGNS = [
    ("Weibull v1", "results_weibull_v1/run_seed_*_group_*.json"),
    ("Weibull v2", "results_weibull_v2/run_seed_*_group_*.json"),
    ("Temoin tirage", "results_weibull_temoin/run_seed_*_tirage.json"),
    ("MiniMax v1 tirage", "results_minimax_v1/tirage/run_seed_*_tirage.json"),
    ("MiniMax v1 evolution", "results_minimax_v1/evolution/run_seed_*_evolution.json"),
    ("MiniMax v2 tirage", "results_minimax_v2/tirage/run_seed_*_tirage.json"),
    ("MiniMax v2 evolution", "results_minimax_v2/evolution/run_seed_*_evolution.json"),
]
SETS = ["In-Dist", "Valid", "OOD-1", "OOD-2"]
LINE = re.compile(r"(\S+)\s*: valid=(\w+)(?: (\d+) bins)?.*?Best-Fit: (\d+) \(excess: \+(\d+)\)")
ENV = dict(os.environ, NOMAD_BP_PROFILE="weibull", NOMAD_FUEL_LIMIT="400000")
TMP = tempfile.mkdtemp(prefix="nomad_reeval_")


def evaluate(exe, seed, code_path):
    out = subprocess.run([exe, "eval", "--seed", str(seed), "--fuel", "400000", "--code", code_path],
                         capture_output=True, text=True, env=ENV).stdout
    res = {}
    for lab, valid, bins, bf, bf_excess in LINE.findall(out):
        res[lab] = {"valid": valid == "true", "bins": int(bins) if bins else None,
                    "bf": int(bf), "l1": int(bf) - int(bf_excess)}
    return res


def effect(r):
    if not r or not r["valid"]:
        return None
    return 100 * (r["bf"] - r["bins"]) / (r["bf"] - r["l1"])


def job(args):
    campaign, path = args
    run = json.load(open(path, encoding="utf-8"))
    code_path = os.path.join(TMP, re.sub(r"\W", "_", path) + ".rs")
    open(code_path, "w", encoding="utf-8").write(run["best_capsule"]["logic_code"])
    seed = run["rng_seed"]
    return {"campaign": campaign, "file": path.replace("\\", "/"), "seed": seed,
            "old": evaluate(OLD, seed, code_path), "new": evaluate(NEW, seed, code_path)}


jobs = [(name, f) for name, pattern in CAMPAIGNS for f in sorted(glob.glob(pattern))]
with ThreadPoolExecutor(max_workers=6) as pool:
    rows = list(pool.map(job, jobs))
if DUMP:
    json.dump(rows, open(DUMP, "w", encoding="utf-8"), indent=1)

fmt = lambda v: "invalide" if v is None else f"{v:6.1f} %"
print("Champions dont le resultat change sur au moins un jeu :")
for r in rows:
    diffs = [lab for lab in SETS
             if (r["old"].get(lab, {}).get("valid"), r["old"].get(lab, {}).get("bins"))
             != (r["new"].get(lab, {}).get("valid"), r["new"].get(lab, {}).get("bins"))]
    r["changed"] = diffs
    if diffs:
        print(f"  {r['file']}")
        for lab in diffs:
            print(f"      {lab:8}: {fmt(effect(r['old'].get(lab)))} -> {fmt(effect(r['new'].get(lab)))}")

print()
print(f"{'Campagne':22} | runs | inchanges | changes | invalides (nouveau, validation) | succes>=10% validation ancien -> nouveau | succes>=30% ancien -> nouveau | mediane ancien -> nouveau")
for name, _ in CAMPAIGNS:
    rs = [r for r in rows if r["campaign"] == name]
    old = [effect(r["old"].get("Valid")) for r in rs]
    new = [effect(r["new"].get("Valid")) for r in rs]
    med = lambda v: statistics.median([x if x is not None else float("-inf") for x in v])
    count = lambda v, t: sum(1 for x in v if x is not None and x >= t)
    print(f"{name:22} | {len(rs):4} | {sum(not r['changed'] for r in rs):9} | {sum(bool(r['changed']) for r in rs):7} |"
          f" {sum(x is None for x in new):31} | {count(old, 10):>18} -> {count(new, 10):<19} |"
          f" {count(old, 30):>13} -> {count(new, 30):<13} | {med(old):6.1f} -> {med(new):6.1f}")
print(f"TOTAL : {len(rows)} champions, {sum(bool(r['changed']) for r in rows)} changes,"
      f" {sum(1 for r in rows if effect(r['new'].get('Valid')) is None)} invalides sur la validation avec le nouvel executable")
