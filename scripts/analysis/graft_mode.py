"""Depouillement du banc greffe avec melanges contre regles completes.
Usage : python scripts/analysis/graft_mode.py mesures/graft_mode
"""
import json, glob, sys
from collections import defaultdict
rows=[json.loads(l) for f in sorted(glob.glob(sys.argv[1]+"/part*.jsonl")) for l in open(f,encoding="utf-8")]
print("paires",len(rows))
for m in ["mixing","rules"]:
    err=sum(1 for r in rows if f"{m}_error" in r)
    ok=[r for r in rows if f"{m}_error" not in r]
    valid=sum(1 for r in ok if r[f"{m}_valid"])
    mixed=sum(1 for r in ok if r.get(f"{m}_mixed"))
    acc=[r for r in ok if r[f"{m}_accepted"]]
    gain=[r["host_bins"]-r[f"{m}_bins"] for r in acc]
    vh=[r for r in acc if r.get(f"{m}_val_valid") and r.get("host_val_valid")]
    vgain=[r["host_val_bins"]-r[f"{m}_val_bins"] for r in vh]
    vhold=sum(1 for g in vgain if g>0); vtie=sum(1 for g in vgain if g==0); vloss=sum(1 for g in vgain if g<0)
    champ=sum(1 for r in acc if r["champion_bins"] is not None and r[f"{m}_bins"]<r["champion_bins"])
    accmixed=sum(1 for r in acc if r.get(f"{m}_mixed"))
    seeds=len({r["seed"] for r in acc})
    print(f"\n== {m}: echecs greffe {err}, enfants {len(ok)}, valides {valid}, avec melange {mixed}")
    print(f"  gardes (battent hote) {len(acc)} ({100*len(acc)/len(rows):.1f}%), dont melanges {accmixed}, seeds touchees {seeds}/12")
    if gain: print(f"  gain train bins: total {sum(gain)}, median {sorted(gain)[len(gain)//2]}, max {max(gain)}")
    print(f"  validation: tient {vhold}, egal {vtie}, perd {vloss}, gain total val {sum(vgain)}")
    print(f"  battent le champion de la seed: {champ}")
    lens=[r[f"{m}_len"]-r["host_len"] for r in ok]
    print(f"  croissance code moyenne {sum(lens)/len(lens):.0f} car.")
both=sum(1 for r in rows if r.get("mixing_accepted") and r.get("rules_accepted"))
mo=sum(1 for r in rows if r.get("mixing_accepted") and not r.get("rules_accepted"))
ro=sum(1 for r in rows if r.get("rules_accepted") and not r.get("mixing_accepted"))
print(f"\nappariement: les deux {both}, mixing seul {mo}, rules seul {ro}")
same=sum(1 for r in rows if r.get("mixing_bins") is not None and r.get("mixing_bins")==r.get("rules_bins"))
print("bins identiques entre modes",same)
# per seed
ps=defaultdict(lambda:[0,0])
for r in rows:
    ps[r["seed"]][0]+=bool(r.get("mixing_accepted")); ps[r["seed"]][1]+=bool(r.get("rules_accepted"))
print("par seed (mixing, rules):",dict(ps))
# orig engine acceptance
print("journal original is_valid:",sum(1 for r in rows if r.get("orig_accepted_journal_valid")))

# --- robustesse ---
for m in ["mixing","rules"]:
    acc=[r for r in rows if r.get(f"{m}_accepted")]
    hi=sum(1 for r in acc if not r["host_valid"])
    g=sorted(r["host_bins"]-r[f"{m}_bins"] for r in acc if r["host_valid"])
    big=sum(1 for x in g if x>100)
    print(m,"hote invalide",hi,"| gains hote valide: n",len(g),"total",sum(g),"sans >100:",sum(x for x in g if x<=100),"nb>100",big, "top5",g[-5:])
    vg=sorted(r["host_val_bins"]-r[f"{m}_val_bins"] for r in acc if r.get(f"{m}_val_valid") and r.get("host_val_valid"))
    print("   val median",vg[len(vg)//2],"total sans >100",sum(x for x in vg if abs(x)<=100))
# per seed sign
w=l=t=0
from collections import Counter
c=Counter()
for r in rows:
    c[(r["seed"],"m")]+=bool(r.get("mixing_accepted")); c[(r["seed"],"r")]+=bool(r.get("rules_accepted"))
for s in {r["seed"] for r in rows}:
    d=c[(s,"r")]-c[(s,"m")]; w+=d>0; l+=d<0; t+=d==0
print("seeds rules>mixing",w,"<",l,"=",t)
# discordant paired gain on same pair where both accepted
both=[r for r in rows if r.get("mixing_accepted") and r.get("rules_accepted")]
print("les deux gardes: rules meilleur",sum(r["rules_bins"]<r["mixing_bins"] for r in both),"mixing meilleur",sum(r["rules_bins"]>r["mixing_bins"] for r in both))
