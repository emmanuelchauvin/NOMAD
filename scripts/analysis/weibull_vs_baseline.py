"""Analyse de la campagne Weibull selon docs/PROTOCOLE_WEIBULL.md.

Compare, seed par seed, le champion de chaque groupe a Best-Fit sur les memes jeux,
et donne le repere haut (heuristique de reference ecrite a la main).

Usage : python scripts/analysis/weibull_vs_baseline.py <dossier_resultats> <nomad.exe>
Fonctionne aussi sur une campagne en cours (seeds terminees seulement).
"""
import glob, itertools, json, os, re, statistics, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SETS = [("Entrainement", "best_in_distribution_score", "In-Dist"),
        ("Validation", "best_validation_score", "Valid"),
        ("OOD-1", "best_ood1_score", "OOD-1"),
        ("OOD-2", "best_ood2_score", "OOD-2")]
DEV_SEEDS = {1000}


def eval_code(exe, seed, code):
    env = dict(os.environ, NOMAD_BP_PROFILE="weibull", NOMAD_FUEL_LIMIT="400000")
    out = subprocess.run([exe, "eval", "--seed", str(seed), "--fuel", "400000", "--code", code],
                         capture_output=True, text=True, env=env).stdout
    res = {}
    for label, bins, exc, bf, bfexc in re.findall(
            r"(\S+)\s*: valid=true (\d+) bins \(excess: \+(\d+)\), Best-Fit: (\d+) \(excess: \+(\d+)\)", out):
        res[label] = dict(bins=int(bins), bf=int(bf), lb=int(bf) - int(bfexc))
    return res


def wilcoxon_exact(diffs, two_sided=False):
    """Wilcoxon signe exact par enumeration des signes. diffs > 0 = favorable."""
    d = [x for x in diffs if x != 0]
    n = len(d)
    if n == 0:
        return 1.0
    order = sorted(range(n), key=lambda i: abs(d[i]))
    ranks = [0.0] * n
    i = 0
    while i < n:
        j = i
        while j + 1 < n and abs(d[order[j + 1]]) == abs(d[order[i]]):
            j += 1
        for k in range(i, j + 1):
            ranks[order[k]] = (i + j) / 2 + 1
        i = j + 1
    w = sum(r for r, x in zip(ranks, d) if x > 0)
    total = sum(ranks)
    ge = le = 0
    for signs in itertools.product((0, 1), repeat=n):
        v = sum(r for r, s in zip(ranks, signs) if s)
        ge += v >= w - 1e-9
        le += v <= w + 1e-9
    m = 2 ** n
    if two_sided:
        return min(1.0, 2 * min(ge, le) / m)
    return ge / m


def holm(ps):
    order = sorted(range(len(ps)), key=lambda i: ps[i])
    adj = [0.0] * len(ps)
    run = 0.0
    for k, i in enumerate(order):
        run = max(run, min(1.0, (len(ps) - k) * ps[i]))
        adj[i] = run
    return adj


def main():
    rdir, exe = sys.argv[1], os.path.abspath(sys.argv[2])
    runs = {}
    for f in glob.glob(os.path.join(rdir, "run_seed_*_group_*.json")):
        m = re.search(r"run_seed_(\d+)_group_([cd])", f)
        r = json.load(open(f, encoding="utf-8"))
        runs.setdefault(int(m.group(1)), {})[m.group(2).upper()] = {
            lab: r[key]["total_bins"] if r[key]["is_valid"] else None for _, key, lab in SETS}
    seeds = sorted(s for s in runs if "C" in runs[s] and "D" in runs[s])
    if not seeds:
        print("Aucune seed complete (C et D) pour l'instant.")
        return
    rows = {}
    for s in seeds:
        ref = eval_code(exe, s, os.path.join(HERE, "heuristics", "reference_weibull.rs"))
        rows[s] = dict(C=runs[s]["C"], D=runs[s]["D"], ref=ref)
    print(f"Seeds completes : {seeds}  (mise au point : {sorted(DEV_SEEDS & set(seeds))})\n")
    for name, _, lab in SETS:
        print(f"== {name}")
        print("  seed   L1    Best-Fit  GroupeC  GroupeD  Reference | exces supprime C   D")
        for s in seeds:
            r = rows[s]; bf = r["ref"][lab]["bf"]; lb = r["ref"][lab]["lb"]
            c, d = r["C"][lab], r["D"][lab]
            fr = lambda v: "   n/a" if v is None else f"{100 * (bf - v) / (bf - lb):5.1f}%"
            print(f"  {s}{'*' if s in DEV_SEEDS else ' '} {lb:6d} {bf:8d} {str(c):>8} {str(d):>8} {r['ref'][lab]['bins']:9d} |"
                  f"            {fr(c)} {fr(d)}")
        print()
    conf = [s for s in seeds if s not in DEV_SEEDS]
    print(f"== Tests du protocole sur les {len(conf)} seeds hors mise au point")
    if not conf:
        return

    def diffs(lab, g):
        return [rows[s]["ref"][lab]["bf"] - (rows[s][g][lab] if rows[s][g][lab] is not None else 10**9)
                for s in conf]

    def effect(lab, g):
        v = [(rows[s]["ref"][lab]["bf"] - rows[s][g][lab]) / (rows[s]["ref"][lab]["bf"] - rows[s]["ref"][lab]["lb"])
             for s in conf if rows[s][g][lab] is not None]
        return (statistics.median(v), min(v), max(v)) if v else (float("nan"),) * 3

    pa = wilcoxon_exact(diffs("Valid", "C"))
    med, lo, hi = effect("Valid", "C")
    wins = sum(x > 0 for x in diffs("Valid", "C"))
    print(f"  H-A  C < Best-Fit sur validation : {wins}/{len(conf)} seeds gagnees, p = {pa:.4f} (unilateral)")
    print(f"       exces de Best-Fit supprime : mediane {100 * med:.1f}% (de {100 * lo:.1f}% a {100 * hi:.1f}%)")
    pb = wilcoxon_exact(diffs("OOD-1", "C")); pc = wilcoxon_exact(diffs("OOD-2", "C"))
    cd = [(rows[s]["D"]["Valid"] or 10**9) - (rows[s]["C"]["Valid"] or 10**9) for s in conf]
    pd_ = wilcoxon_exact(cd, two_sided=True)
    adj = holm([pb, pc, pd_])
    for lab, name, p, a in (("OOD-1", "H-B  C < Best-Fit sur OOD-1     ", pb, adj[0]),
                            ("OOD-2", "H-C  C < Best-Fit sur OOD-2     ", pc, adj[1])):
        med, lo, hi = effect(lab, "C")
        print(f"  {name}: {sum(x > 0 for x in diffs(lab, 'C'))}/{len(conf)} gagnees, p = {p:.4f}, Holm = {a:.4f},"
              f" exces supprime mediane {100 * med:.1f}%")
    print(f"  H-D  C contre D sur validation   : C gagne {sum(x > 0 for x in cd)}, D gagne {sum(x < 0 for x in cd)},"
          f" egalites {sum(x == 0 for x in cd)}, p = {pd_:.4f} (bilateral), Holm = {adj[2]:.4f}")
    med, lo, hi = effect("Valid", "D")
    wd = sum(x > 0 for x in diffs("Valid", "D"))
    print(f"  (descriptif) D < Best-Fit sur validation : {wd}/{len(conf)}, exces supprime mediane {100 * med:.1f}%")


if __name__ == "__main__":
    main()
