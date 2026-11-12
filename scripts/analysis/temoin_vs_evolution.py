"""Compare le temoin "tirage sans evolution" aux groupes C et D de la campagne Weibull v2.

Voir docs/PROTOCOLE_WEIBULL_TEMOIN.md. Usage : python scripts/analysis/temoin_vs_evolution.py
"""
import json, os, re, subprocess, statistics
exe=os.path.abspath("target/frozen/nomad-campaign-weibull-v2.exe")
SETS=[("Entrainement","best_in_distribution_score","In-Dist"),("Validation","best_validation_score","Valid"),("OOD-1","best_ood1_score","OOD-1"),("OOD-2","best_ood2_score","OOD-2")]
def bf(seed):
    env=dict(os.environ,NOMAD_BP_PROFILE="weibull",NOMAD_FUEL_LIMIT="400000")
    out=subprocess.run([exe,"eval","--seed",str(seed),"--fuel","400000","--code","scripts/analysis/heuristics/best_fit.rs"],capture_output=True,text=True,env=env).stdout
    return {l:(int(b),int(b)-int(e)) for l,b,e in re.findall(r"(\S+)\s*: valid=true \d+ bins \(excess: \+\d+\), Best-Fit: (\d+) \(excess: \+(\d+)\)",out)}
def eff(run,key,lab,ref):
    s=run[key]
    if not s["is_valid"]: return None
    b,l=ref[lab]; return 100*(b-s["total_bins"])/(b-l)
res={}
for seed in range(2000,2188,17):
    ref=bf(seed)
    runs={"T":json.load(open(f"results_weibull_temoin/run_seed_{seed}_tirage.json",encoding="utf-8")),
          "C":json.load(open(f"results_weibull_v2/run_seed_{seed}_group_c.json",encoding="utf-8")),
          "D":json.load(open(f"results_weibull_v2/run_seed_{seed}_group_d.json",encoding="utf-8"))}
    res[seed]={g:{lab:eff(r,key,lab,ref) for _,key,lab in SETS} for g,r in runs.items()}
    res[seed]["calls"]=runs["T"]["budget_consumed"]["llm_calls_consumed"]; res[seed]["name"]=runs["T"]["best_capsule"]["name"]
f=lambda v:"   n/a" if v is None else f"{v:6.1f}"
for name,_,lab in SETS:
    print("==",name,"(exces de Best-Fit supprime, %)  seed | Tirage | C | D")
    for s in res: print(f"  {s} | {f(res[s]['T'][lab])} | {f(res[s]['C'][lab])} | {f(res[s]['D'][lab])}")
    for g in "TCD":
        v=[res[s][g][lab] for s in res if res[s][g][lab] is not None]
        print(f"   {g}: mediane {statistics.median(v):.1f}  succes>=10%: {sum(x>=10 for x in v)}/12  invalides: {12-len(v)}")
print("appels:",{s:res[s]["calls"] for s in res})
