"""Compare l'interpreteur de NOMAD au vrai compilateur Rust sur le code des champions.

Pour chaque champion des campagnes Weibull et MiniMax :
  1. compile sa fonction `score_bin` avec rustc (mode debug : les depassements paniquent) ;
  2. l'execute sur des contextes tires au hasard ;
  3. compare, contexte par contexte, au score de l'interpreteur.

Un code que rustc refuse est compte a part : l'interpreteur est plus tolerant que Rust
(par exemple sur le melange d'entiers et de flottants).

Usage : python scripts/analysis/diff_interpreter_vs_rustc.py [nombre de contextes]
"""
import glob, json, math, os, random, re, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor

N = int(sys.argv[1]) if len(sys.argv) > 1 else 400
PATTERNS = ["results_weibull_v1/run_seed_*_group_*.json", "results_weibull_v2/run_seed_*_group_*.json",
            "results_weibull_temoin/run_seed_*_tirage.json", "results_minimax_v1/*/run_seed_*.json",
            "results_minimax_v2/*/run_seed_*.json"]
FIELDS = ["item_size", "next_item_size", "bin_residual", "bin_capacity", "step_index",
          "total_items", "open_bins_count", "mean_residual", "post_residual", "fitting_bins_count"]
MAIN = """
fn main() {
    use std::io::BufRead;
    std::panic::set_hook(Box::new(|_| {}));
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 { continue; }
        let item_size: u32 = f[0].parse().unwrap();
        let next_item_size: u32 = f[1].parse().unwrap();
        let bin_residual: u32 = f[2].parse().unwrap();
        let bin_capacity: u32 = f[3].parse().unwrap();
        let step_index: usize = f[4].parse().unwrap();
        let total_items: usize = f[5].parse().unwrap();
        let open_bins_count: usize = f[6].parse().unwrap();
        let mean_residual: f64 = f[7].parse().unwrap();
        let post_residual: u32 = f[8].parse().unwrap();
        let fitting_bins_count: usize = f[9].parse().unwrap();
        match std::panic::catch_unwind(|| score_bin(ARGS)) {
            Ok(s) => println!("{:?}", s),
            Err(_) => println!("PANIC"),
        }
    }
}
"""

random.seed(20261113)
work = tempfile.mkdtemp(prefix="nomad_diff_")
contexts = []
for i in range(N):
    cap = 100
    item = random.randint(1, 100)
    residual = cap if i % 10 == 0 else random.randint(item, cap)
    total = random.choice([1000, 5000])
    opened = random.randint(1, 600)
    contexts.append([item, random.randint(0, 100), residual, cap, random.randint(0, total - 1), total,
                     opened, round(random.uniform(0, 100), 6), residual - item, random.randint(1, opened)])
open(f"{work}/contexts.txt", "w").write("\n".join(" ".join(map(str, c)) for c in contexts) + "\n")

files = sorted(f for p in PATTERNS for f in glob.glob(p))
champions = []
for i, path in enumerate(files):
    code = json.load(open(path, encoding="utf-8"))["best_capsule"]["logic_code"]
    signature = code[:code.index("{")]
    params = re.findall(r"([A-Za-z_]\w*)\s*:", signature)
    champions.append({"file": path.replace("\\", "/"), "id": f"c{i:03}", "params": params, "code": code})
    open(f"{work}/c{i:03}.code.rs", "w", encoding="utf-8").write(code)


def native(ch):
    if any(p.lstrip("_") not in FIELDS for p in ch["params"]):
        return "signature"
    src = f"{work}/{ch['id']}.native.rs"
    exe = f"{work}/{ch['id']}.exe"
    open(src, "w", encoding="utf-8").write(
        "#![allow(warnings)]\n" + ch["code"] + MAIN.replace("ARGS", ", ".join(p.lstrip("_") for p in ch["params"])))
    build = subprocess.run(["rustc", "--edition", "2021", "--crate-name", ch["id"], "-o", exe, src], capture_output=True, text=True)
    if build.returncode != 0:
        errors = re.findall(r"^error(?:\[E\d+\])?: (.*)$", build.stderr, flags=re.M)
        ch["compile_error"] = errors[0] if errors else "erreur"
        return "refuse"
    out = subprocess.run([exe], input=open(f"{work}/contexts.txt").read(), capture_output=True, text=True)
    ch["native"] = out.stdout.split("\n")[:N]
    return "ok"


with ThreadPoolExecutor(max_workers=6) as pool:
    statuses = list(pool.map(native, champions))

subprocess.run(["cargo", "test", "--release", "-p", "nomad-sandbox", "differential_dump", "--", "--ignored"],
               env=dict(os.environ, NOMAD_DIFF_DIR=work), capture_output=True, text=True, check=True)


def same(n, i):
    if n == "PANIC" or n in ("inf", "-inf", "NaN"):
        return i.startswith("ERR")
    if i.startswith("ERR"):
        return False
    a, b = float(n), float(i)
    return a == b or math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12)


compiled = agree = 0
print(f"{len(champions)} champions, {N} contextes chacun")
for ch, status in zip(champions, statuses):
    interp = open(f"{work}/{ch['id']}.interp.txt").read().split("\n")
    if status != "ok":
        continue
    compiled += 1
    bad = [k for k in range(N) if not same(ch["native"][k], interp[k])]
    if bad:
        k = bad[0]
        print(f"  ECART {ch['file']} : {len(bad)}/{N} contextes ; ex. {contexts[k]} rustc={ch['native'][k]} interpreteur={interp[k]}")
    else:
        agree += 1
refused = [ch for ch, s in zip(champions, statuses) if s == "refuse"]
print("Signature non canonique :", [ch["file"] for ch, s in zip(champions, statuses) if s == "signature"])
print(f"Compiles par rustc : {compiled}/{len(champions)}")
print(f"  dont identiques a l'interpreteur sur tous les contextes : {agree}/{compiled}")
print(f"Refuses par rustc : {len(refused)}")
reasons = {}
for ch in refused:
    key = re.sub(r"`[^`]*`", "`..`", ch["compile_error"])
    reasons[key] = reasons.get(key, 0) + 1
for key, n in sorted(reasons.items(), key=lambda kv: -kv[1]):
    print(f"    {n:3} x {key}")
print("dossier de travail :", work)
