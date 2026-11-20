"""Depouille le protocole docs/PROTOCOLE_GEMINI_COURBE.md.

Usage : python scripts/analysis/gemini_courbe.py [controle] [--seeds24 | --glm | --glm340]
  --glm     : depouille docs/PROTOCOLE_GLM_COURBE.md (memes calculs, autre dossier, autres
              seeds, autres prix).
  --glm340  : depouille docs/PROTOCOLE_GLM_340.md (memes calculs, budget de 344 appels,
              paliers 85, 170 et 344, pas de test H3).
  controle  : fait tous les calculs mais n'affiche que des controles de coherence, aucun
              score (pour verifier le script avant le depouillement).
  --seeds24 : depouillement de l'extension prevue (24 seeds, seuil 0,025).
"""
import glob, json, os, re, statistics, subprocess, sys

EXE = os.path.abspath("target/frozen/nomad-gemini-courbe.exe")
OUT = "results_gemini_courbe"
ARMS = ["evolution", "tirage"]
N_SEEDS = 24 if "--seeds24" in sys.argv else 12
SEEDS = [8000 + 17 * r for r in range(N_SEEDS)]
ALPHA = 0.025 if N_SEEDS == 24 else 0.05
BUDGET = 1359
PALIERS = [85, 170, 340, 680, 1359]
INIT_POP = 22  # propositions fraiches du depart de l'evolution
PRICE_IN, PRICE_OUT = 0.25, 1.50  # dollars par million de tokens, gemini-3.1-flash-lite
GLM = "--glm" in sys.argv
if GLM:
    # docs/PROTOCOLE_GLM_COURBE.md : 24 seeds d'emblee, un seul regard, seuil 0,05.
    OUT = "results_glm_courbe"
    N_SEEDS = 24
    SEEDS = [9000 + 17 * r for r in range(N_SEEDS)]
    ALPHA = 0.05
    PRICE_IN, PRICE_OUT = 0.0399, 0.0784  # z-ai/glm-5.3-flash chez Relace (OpenRouter)
GLM340 = "--glm340" in sys.argv
if GLM340:
    # docs/PROTOCOLE_GLM_340.md : 24 seeds, un seul regard, seuil 0,05, 344 appels, sans H3.
    OUT = "results_glm_340"
    N_SEEDS = 24
    SEEDS = [9000 + 17 * r for r in range(N_SEEDS)]
    ALPHA = 0.05
    BUDGET = 344
    PALIERS = [85, 170, 344]
    PRICE_IN, PRICE_OUT = 0.0399, 0.0784  # z-ai/glm-5.3-flash chez Relace (OpenRouter)
INVALID = -100.0  # effet attribue a un champion invalide (protocole)
CONTROLE = "controle" in sys.argv
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


def champions(report):
    """Liste croissante des (indice, proposition) marquees is_new_best."""
    return [(p["index"], p) for p in report["proposals"] if p["is_new_best"]]


def champion_at(champs, k):
    """Champion apres k propositions : la derniere is_new_best parmi les k premieres."""
    last = None
    for i, p in champs:
        if i > k:
            break
        last = p
    return last


def effects(p, ref):
    """(effet validation, effet entrainement) d'un champion ; invalide = -100 %."""
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


# ---------------------------------------------------------------- chargement
missing = [f"{a}/{s}" for a in ARMS for s in SEEDS
           if not os.path.exists(f"{OUT}/{a}/run_seed_{s}_{a}.json")]
if missing:
    sys.exit(f"Rapports manquants : {missing}")

R = {s: refs(s) for s in SEEDS}
reports = {a: {s: json.load(open(f"{OUT}/{a}/run_seed_{s}_{a}.json", encoding="utf-8"))
               for s in SEEDS} for a in ARMS}
champs = {a: {s: champions(reports[a][s]) for s in SEEDS} for a in ARMS}
# effet[a][s][k] = (validation, entrainement) du champion apres k propositions
curve = {a: {s: {k: effects(champion_at(champs[a][s], k), R[s]) for k in range(1, BUDGET + 1)}
             for s in SEEDS} for a in ARMS}

# ---------------------------------------------------------------- controles
problems = []
for a in ARMS:
    for s in SEEDS:
        rep = reports[a][s]
        P = rep["proposals"]
        if rep["budget_consumed"]["llm_calls_consumed"] != BUDGET or len(P) != BUDGET:
            problems.append(f"{a}/{s} : budget ou journal incomplet")
        if any(p["index"] != i + 1 or p["llm_calls_consumed"] != i + 1 for i, p in enumerate(P)):
            problems.append(f"{a}/{s} : ordre du journal")
        last = champion_at(champs[a][s], BUDGET)
        if last is None or last["capsule_id"] != rep["best_capsule"]["id"]:
            problems.append(f"{a}/{s} : champion final different du rapport")
            continue
        if last["total_bins"] != rep["best_in_distribution_score"]["total_bins"]:
            problems.append(f"{a}/{s} : bacs d'entrainement du champion")
        if (last["validation_is_valid"] != rep["best_validation_score"]["is_valid"]
                or last["validation_bins"] != rep["best_validation_score"]["total_bins"]):
            problems.append(f"{a}/{s} : validation du champion")
        if any(p["validation_bins"] is None for _, p in champs[a][s]):
            problems.append(f"{a}/{s} : champion sans note de validation")
        # Le champion a l'entrainement ne doit jamais reculer.
        tr = [curve[a][s][k][1] for k in range(1, BUDGET + 1) if curve[a][s][k][1] != INVALID]
        if any(y < x - 1e-9 for x, y in zip(tr, tr[1:])):
            problems.append(f"{a}/{s} : le champion recule a l'entrainement")
        if usage(a, s)[0] != BUDGET:
            problems.append(f"{a}/{s} : journal d'usage incomplet")
        if any(curve[a][s][k] == (INVALID, INVALID) for k in PALIERS):
            problems.append(f"{a}/{s} : aucun champion a un palier")

print(f"Controles : {N_SEEDS} seeds x 2 bras, {len(problems)} probleme(s)")
for p in problems:
    print("   ", p)
if CONTROLE:
    print("Mode controle : aucun score affiche.")
    sys.exit(1 if problems else 0)

# ---------------------------------------------------------------- descriptif
V = lambda a, s, k: curve[a][s][k][0]
T = lambda a, s, k: curve[a][s][k][1]
fmt = lambda v: f"{v:7.1f}"

for title, f in (("Validation", V), ("Entrainement", T)):
    print(f"\n== {title} : effet (part de l'exces de Best-Fit supprimee, %) par palier")
    print("   seed  | " + " | ".join(f"evo {k:4} / tir {k:4}" for k in PALIERS))
    for s in SEEDS:
        print(f"   {s}  | " + " | ".join(f"{fmt(f('evolution', s, k))} / {fmt(f('tirage', s, k))}" for k in PALIERS))
    for a in ARMS:
        print(f"   {a:9} :")
        for k in PALIERS:
            v = [f(a, s, k) for s in SEEDS]
            print(f"      {k:4} appels : mediane {med(v):6.1f}  >=10% : {sum(x >= 10 for x in v):2}/{N_SEEDS}"
                  f"  >=30% : {sum(x >= 30 for x in v):2}/{N_SEEDS}  invalides : {sum(x == INVALID for x in v)}")

print("\n== Ecart evolution - tirage sur la validation, par palier")
gap = {k: [V("evolution", s, k) - V("tirage", s, k) for s in SEEDS] for k in PALIERS}
for k in PALIERS:
    g = gap[k]
    print(f"   {k:4} appels : ecart median {med(g):6.1f} points, seeds gagnees par l'evolution"
          f" {sum(x > 0 for x in g):2}, perdues {sum(x < 0 for x in g):2}, egalites {sum(x == 0 for x in g):2}")

print("\n== Budget de tirage equivalent (validation, medianes)")
tir_med = [med(V("tirage", s, n) for s in SEEDS) for n in range(1, BUDGET + 1)]
for k in PALIERS:
    target = med(V("evolution", s, k) for s in SEEDS)
    n = next((i + 1 for i, m in enumerate(tir_med) if m >= target), None)
    print(f"   evolution a {k:4} appels (mediane {target:5.1f}) : tirage de "
          + (f"{n} propositions" if n else f"plus de {BUDGET} propositions"))

print("\n== Dernier appel ayant ameliore le champion, par seed")
for a in ARMS:
    last = [champs[a][s][-1][0] for s in SEEDS]
    print(f"   {a:9} : {last}  mediane {med(last)}")

print("\n== Champions de l'evolution issus d'une proposition fraiche de relance (indice > 22)")
for k in PALIERS:
    n = sum(1 for s in SEEDS if (lambda p: p["kind"] == "fresh" and p["index"] > INIT_POP)(
        champion_at(champs["evolution"][s], k)))
    print(f"   {k:4} appels : {n}/{N_SEEDS}")
kinds = {}
for s in SEEDS:
    for p in reports["evolution"][s]["proposals"]:
        kinds[p["kind"]] = kinds.get(p["kind"], 0) + 1
print(f"   propositions de l'evolution par type : {kinds}")

print("\n== Propositions invalides a l'entrainement, par bras")
for a in ARMS:
    P = [p for s in SEEDS for p in reports[a][s]["proposals"]]
    print(f"   {a:9} : {sum(not p['is_valid'] for p in P)}/{len(P)}"
          f" ({100 * sum(not p['is_valid'] for p in P) / len(P):.1f} %)")

print("\n== Tokens et cout")
per_call = {}
for a in ARMS:
    tot = [usage(a, s) for s in SEEDS]
    calls, p, c = sum(t[0] for t in tot), sum(t[1] for t in tot), sum(t[2] for t in tot)
    per_call[a] = cost(p, c) / calls
    print(f"   {a:9} : requetes {calls}, tokens entree {p}, sortie {c}, cout estime {cost(p, c):.2f} $,"
          f" {per_call[a]:.6f} $ par appel")
r = per_call["evolution"] / per_call["tirage"]
k_star = int(BUDGET // r)
print(f"   rapport de cout r = {r:.4f}, k* = floor({BUDGET} / r) = {k_star}")

# ---------------------------------------------------------------- tests
def test(label, a, b, extra=""):
    w, n, p = wilcoxon_exact(a, b)
    up, down = sum(x > y for x, y in zip(a, b)), sum(x < y for x, y in zip(a, b))
    print(f"\n== {label}{extra}")
    print(f"   en faveur {up}, en defaveur {down}, egalites {len(a) - up - down};"
          f" difference mediane {med(x - y for x, y in zip(a, b)):.1f} points")
    print(f"   Wilcoxon signe apparie bilateral exact : W+ = {w}, n = {n}, p = {p:.4f}  (seuil {ALPHA})")
    return p, up, down

if not GLM340:
    p3, up3, down3 = test("H3 (echelle) : ecart a 1359 appels contre ecart a 340 appels", gap[1359], gap[340])
    h3 = p3 < ALPHA and up3 > down3
    print("   H3 " + ("ETABLIE" if h3 else ("significative dans l'autre sens" if p3 < ALPHA else "non etablie")))
    if not h3 and N_SEEDS == 12 and not GLM:
        ext = p3 < 0.20 and up3 > down3
        print("   Extension prevue (12 seeds de plus, seuil 0,025) : " + ("AUTORISEE" if ext else "non autorisee"))

e1359 = [V("evolution", s, BUDGET) for s in SEEDS]
t1359 = [V("tirage", s, BUDGET) for s in SEEDS]
p1, up1, down1 = test(f"H1 (appels egaux) : evolution contre tirage a {BUDGET} appels", e1359, t1359)
h1 = p1 < ALPHA and up1 > down1
print("   H1 " + ("ETABLIE" if h1 else "non etablie"))

ek = [V("evolution", s, min(k_star, BUDGET)) for s in SEEDS]
p2, up2, down2 = test(f"H2 (depense egale) : evolution a {k_star} appels contre tirage a {BUDGET}", ek, t1359,
                      "" if h1 else "  -- non interpretee, H1 n'est pas etablie")
if h1:
    print("   H2 " + ("ETABLIE" if p2 < ALPHA and up2 > down2 else "non etablie"))
