"""Depouille docs/PROTOCOLE_COURBE_ECHELLE.md (aucun run, relit les rapports existants).

Usage : python scripts/analysis/gemini_courbe_echelle.py
"""
import json, os, re, statistics, subprocess

EXE = os.path.abspath("target/frozen/nomad-gemini-echelle.exe")
S85 = [6000 + 17 * r for r in range(24)]
S340 = [7000 + 17 * r for r in range(24)]


def ref_train(seed):
    """Bacs de Best-Fit et borne L1 sur le jeu d'entrainement de la seed."""
    env = dict(os.environ, NOMAD_BP_PROFILE="weibull", NOMAD_FUEL_LIMIT="400000")
    out = subprocess.run([EXE, "eval", "--seed", str(seed), "--fuel", "400000", "--code",
                          "scripts/analysis/heuristics/best_fit.rs"],
                         capture_output=True, text=True, env=env).stdout
    found = dict((lab, (int(b), int(b) - int(e))) for lab, b, e in re.findall(
        r"(\S+)\s*: valid=true \d+ bins \(excess: \+\d+\), Best-Fit: (\d+) \(excess: \+(\d+)\)", out))
    return found["In-Dist"]


def run(out, arm, seed):
    return json.load(open(f"{out}/{arm}/run_seed_{seed}_{arm}.json", encoding="utf-8"))


def eff(bins, ref):
    bf, l1 = ref
    return 100 * (bf - bins) / (bf - l1)


def curve(report, ref):
    """{appels: effet du meilleur individu rencontre jusque-la}."""
    best, out = None, {}
    for g in report["generations_history"]:
        best = g["best_bins"] if best is None else min(best, g["best_bins"])
        out[g["llm_calls_consumed"]] = eff(best, ref)
    return out


refs = {s: ref_train(s) for s in S85 + S340}
evo340 = {s: curve(run("results_gemini_echelle", "evolution340", s), refs[s]) for s in S340}
evo85 = {s: curve(run("results_gemini_v1", "evolution", s), refs[s]) for s in S85}

# Controle : le dernier point de la courbe doit etre le score d'entrainement du champion.
for s in S340:
    final = run("results_gemini_echelle", "evolution340", s)["best_in_distribution_score"]
    assert final["is_valid"] and abs(evo340[s][340] - eff(final["total_bins"], refs[s])) < 1e-9, s

print("== Evolution 340 : effet a l'entrainement (%) par seed, a 88, 172, 256 et 340 appels")
for s in S340:
    c = evo340[s]
    print(f"  {s} | {c[88]:6.1f} | {c[172]:6.1f} | {c[256]:6.1f} | {c[340]:6.1f}")

print("== M1 : progression par tranche de 84 appels")
for a, b in ((88, 172), (172, 256), (256, 340)):
    gains = [evo340[s][b] - evo340[s][a] for s in S340]
    print(f"   de {a} a {b} appels : {sum(g > 1e-9 for g in gains):2}/24 seeds progressent,"
          f" gain median {statistics.median(gains):5.1f} points, gain moyen {statistics.mean(gains):5.1f} points")

print("== M2 : effet median a l'entrainement selon le nombre d'appels")
med = statistics.median
tirage = {
    22: med(evo85[s][22] for s in S85),
    85: med(eff(run("results_gemini_v1", "tirage85", s)["best_in_distribution_score"]["total_bins"], refs[s]) for s in S85),
    88: med(evo340[s][88] for s in S340),
    170: med(eff(run("results_gemini_v1", "tirage170", s)["best_in_distribution_score"]["total_bins"], refs[s]) for s in S85),
    340: med(eff(run("results_gemini_echelle", "tirage340", s)["best_in_distribution_score"]["total_bins"], refs[s]) for s in S340),
    455: med(eff(run("results_gemini_echelle", "tirageDepense", s)["best_in_distribution_score"]["total_bins"], refs[s]) for s in S340),
}
for k, v in tirage.items():
    print(f"   tirage    {k:3} propositions : {v:5.1f}")
for k in (88, 172, 256, 340):
    print(f"   evolution {k:3} appels       : {med(evo340[s][k] for s in S340):5.1f}")
print(f"   evolution  85 appels (seeds 6000) : {med(evo85[s][85] for s in S85):5.1f}")

print("== Dernier appel ayant ameliore le champion (evolution 340), par seed")
last = []
for s in S340:
    c = sorted(evo340[s].items())
    k = max(calls for (calls, v), (_, prev) in zip(c[1:], c[:-1]) if v > prev + 1e-9) if c[-1][1] > c[0][1] + 1e-9 else 88
    last.append(k)
print("  ", sorted(last), "mediane", med(last))
