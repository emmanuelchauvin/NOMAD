"""Depouille le protocole docs/PROTOCOLE_GLM_H3.md.

Deux bras sur glm-5.3-flash, meme executable : `evolution` (groupe D + elagage du
champion) et `tirage` (1 359 propositions fraiches).

Usage : python scripts/analysis/glm_h3.py [controle] [--essai]
  controle : fait tous les calculs mais n'affiche que des controles de coherence, aucun
             score (pour verifier le script avant le depouillement).
  --essai  : verifie le script sur l'essai de fumee (seed 3153, 29 appels).

Le champion apres k appels est la derniere proposition marquee is_new_best dont le nombre
d'appels deja consommes (llm_calls_consumed) est au plus k : les elagages, gratuits,
comptent au palier de l'appel qui les precede.
"""
import glob, json, os, re, statistics, subprocess, sys

EXE = os.path.abspath("target/frozen/nomad-h2-elagage.exe")
OUT = "results_glm_h3"
ARMS = ["evolution", "tirage"]
if "--essai" in sys.argv:
    OUT, SEEDS, BUDGET = "results_glm_h3_essai", [3153], 29
else:
    SEEDS, BUDGET = [10400 + 17 * r for r in range(24)], 1359
ALPHA = 0.05
REF = 344  # palier de reference de H3
PALIERS = [k for k in (85, 170, 344, 680, 1359) if k <= BUDGET] or [BUDGET]
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

per_call, tok = {}, {}
for a in ARMS:
    calls = prompt = completion = 0
    for s in SEEDS:
        c, pi, co = usage(a, s)
        calls, prompt, completion = calls + c, prompt + pi, completion + co
    tok[a] = (calls, prompt, completion)
    per_call[a] = cost(prompt, completion) / max(calls, 1)
r = per_call["evolution"] / per_call["tirage"]
k_star = int(BUDGET // r)
KS = sorted(set(PALIERS + [k_star]))
curve = {a: {s: {k: effects(champion_at(reports[a][s], k), R[s]) for k in KS}
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
        if a == "tirage" and any(p["kind"] != "fresh" for p in props):
            problems.append(f"{a}/{s} : propositions autres que fraiches dans le tirage")
        if a == "evolution" and any(p["kind"] == "crossover" for p in props):
            problems.append(f"{a}/{s} : croisement dans l'evolution")
        if a == "evolution" and rep.get("local_ops", {}).get("prune_attempts", 0) == 0:
            problems.append(f"{a}/{s} : aucun elagage tente")
        for p in props:
            if p["is_new_best"] and p.get("validation_bins") is None:
                problems.append(f"{a}/{s} : champion {p['index']} sans note de validation")
                break
print(f"Controles : {len(SEEDS)} seeds x {len(ARMS)} bras, {len(problems)} probleme(s)")
for p in problems:
    print("  ", p)
total_cost = sum(cost(t[1], t[2]) for t in tok.values())
print(f"Cout total {total_cost:.2f} $")
if CONTROLE:
    print("Mode controle : aucun score affiche.")
    sys.exit(0)

# ---------------------------------------------------------------- tableaux
for idx, label in [(0, "Validation"), (1, "Entrainement")]:
    print(f"\n== {label} : effet (part de l'exces de Best-Fit supprimee, %) par palier")
    print("   seed   | " + " | ".join(f"evo {k:4d} / tir {k:4d}" for k in PALIERS))
    for s in SEEDS:
        cells = [f"{curve['evolution'][s][k][idx]:8.1f} / {curve['tirage'][s][k][idx]:8.1f}" for k in PALIERS]
        print(f"   {s}  | " + " | ".join(cells))
    for a in ARMS:
        print(f"   {a} :")
        for k in PALIERS:
            v = [curve[a][s][k][idx] for s in SEEDS]
            print(f"      {k:4d} appels : mediane {med(v):6.1f}  >=10% : {sum(x >= 10 for x in v):2d}/{len(v)}"
                  f"  >=30% : {sum(x >= 30 for x in v):2d}/{len(v)}  invalides : {sum(x == INVALID for x in v)}")

print("\n== Ecart evolution - tirage sur la validation, a appels egaux")
for k in PALIERS:
    d = [curve["evolution"][s][k][0] - curve["tirage"][s][k][0] for s in SEEDS]
    print(f"   {k:4d} appels : ecart median {med(d):6.1f} points, seeds gagnees par l'evolution "
          f"{sum(x > 0 for x in d):2d}, perdues {sum(x < 0 for x in d):2d}, egalites {sum(x == 0 for x in d):2d}")

print("\n== Exces sur la borne L1 (validation, mediane, % de L1)")
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
print(f"   Best-Fit : {med(100 * (R[s]['Valid'][0] - R[s]['Valid'][1]) / R[s]['Valid'][1] for s in SEEDS):5.2f} %")

print("\n== Propositions invalides a l'entrainement")
for a in ARMS:
    llm = [p for s in SEEDS for p in reports[a][s]["proposals"] if p["kind"] in LLM_KINDS]
    bad = sum(not p["is_valid"] for p in llm)
    print(f"   {a} : {bad}/{len(llm)} ({100 * bad / len(llm):.1f} %)")

print("\n== Elagage (bras evolution, totaux)")
tot = {}
for s in SEEDS:
    for key, v in reports["evolution"][s].get("local_ops", {}).items():
        tot[key] = tot.get(key, 0) + v
print("  ", tot)
print("   champion final issu d'un elagage :",
      sum(champion_at(reports["evolution"][s], BUDGET)["kind"] == "prune" for s in SEEDS), f"/ {len(SEEDS)}")

print("\n== Taille du code du champion final (caracteres, mediane)")
for a in ARMS:
    print(f"   {a} : {med(len(champion_at(reports[a][s], BUDGET)['logic_code']) for s in SEEDS):.0f}")

print("\n== Tokens et cout")
for a in ARMS:
    c, pi, co = tok[a]
    print(f"   {a} : requetes {c}, tokens entree {pi}, sortie {co}, cout {cost(pi, co):.2f} $, "
          f"{per_call[a]:.6f} $ par appel")
print(f"   r = {r:.4f}, k* = floor({BUDGET} / r) = {k_star}")
print("   cout par appel selon le rang de l'appel dans le run :")
for a in ARMS:
    cells, lo = [], 0
    for hi in PALIERS:
        n = pi = co = 0
        for s in SEEDS:
            rows = []
            for path in sorted(glob.glob(f"{OUT}/{a}/usage_seed_{s}_essai*.jsonl")):
                rows = [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()]
            for row in rows[lo:hi]:
                n, pi, co = n + 1, pi + (row.get("prompt_tokens") or 0), co + (row.get("completion_tokens") or 0)
        cells.append(f"{lo + 1}-{hi} : {cost(pi, co) / max(n, 1):.6f} $")
        lo = hi
    print(f"      {a} : " + ", ".join(cells))


def test(label, ka, kb):
    a = [curve["evolution"][s][ka][0] for s in SEEDS]
    b = [curve["tirage"][s][kb][0] for s in SEEDS]
    d = [x - y for x, y in zip(a, b)]
    wins, losses = sum(x > 0 for x in d), sum(x < 0 for x in d)
    w, n, p = wilcoxon_exact(a, b)
    print(f"\n== {label} : evolution a {ka} appels contre tirage a {kb}")
    print(f"   en faveur {wins}, en defaveur {losses}, egalites {len(d) - wins - losses}; "
          f"difference mediane {med(d):.1f} points")
    print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w}, n = {n}, p = {p:.4f}  (seuil {ALPHA})")
    if p < ALPHA and wins > losses:
        print(f"   {label} ETABLIE")
        return True
    if p < ALPHA and losses > wins:
        print("   EFFET SIGNIFICATIF EN SENS CONTRAIRE")
    else:
        print(f"   {label} non etablie")
    return False


if BUDGET > REF:
    hi = [curve["evolution"][s][BUDGET][0] - curve["tirage"][s][BUDGET][0] for s in SEEDS]
    lo = [curve["evolution"][s][REF][0] - curve["tirage"][s][REF][0] for s in SEEDS]
    d = [x - y for x, y in zip(hi, lo)]
    up, down = sum(x > 0 for x in d), sum(x < 0 for x in d)
    w, n, p = wilcoxon_exact(hi, lo)
    print(f"\n== H3 (test principal) : ecart evolution - tirage a {BUDGET} appels contre ecart a {REF}")
    print(f"   ecart median a {REF} : {med(lo):.1f} points, a {BUDGET} : {med(hi):.1f} points")
    print(f"   l'ecart grandit sur {up} seeds, se reduit sur {down}, egalites {len(d) - up - down}; "
          f"variation mediane {med(d):.1f} points")
    print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w}, n = {n}, p = {p:.4f}  (seuil {ALPHA})")
    if p < ALPHA and up > down:
        print("   H3 ETABLIE")
    elif p < ALPHA and down > up:
        print("   EFFET SIGNIFICATIF EN SENS CONTRAIRE")
    else:
        print("   H3 non etablie")

h1 = test("H1", BUDGET, BUDGET)
h2 = test("H2", k_star, BUDGET)
if not h1:
    print("   (H2 n'est interpretee que si H1 est etablie)")
