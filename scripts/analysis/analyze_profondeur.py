import json
from datetime import datetime

with open("comparison_profondeur.json", encoding="utf-8") as f:
    data = json.load(f)

print("=" * 85)
print("     ANALYSE COMPLÈTE DU RUN EN PROFONDEUR (20 Individus, 15 Générations)")
print("=" * 85)
print(f"Nombre total de paires : {len(data)}")

total_gain_fuel = 0
total_pairs_with_gain = 0

for i, (c, d) in enumerate(data):
    seed = c["rng_seed"]
    bins_c = c["best_in_distribution_score"]["total_bins"]
    bins_d = d["best_in_distribution_score"]["total_bins"]
    fuel_c = c["best_in_distribution_score"]["total_fuel"]
    fuel_d = d["best_in_distribution_score"]["total_fuel"]
    
    ood1_c = c["best_ood1_score"]["total_bins"]
    ood1_d = d["best_ood1_score"]["total_bins"]
    ood1_fuel_c = c["best_ood1_score"]["total_fuel"]
    ood1_fuel_d = d["best_ood1_score"]["total_fuel"]

    ood2_c = c["best_ood2_score"]["total_bins"]
    ood2_d = d["best_ood2_score"]["total_bins"]
    ood2_fuel_c = c["best_ood2_score"]["total_fuel"]
    ood2_fuel_d = d["best_ood2_score"]["total_fuel"]

    cap_c = c["best_capsule"]
    cap_d = d["best_capsule"]

    diff_fuel = fuel_d - fuel_c
    pct_fuel = (diff_fuel / fuel_d) * 100 if fuel_d > 0 else 0
    if diff_fuel > 0:
        total_gain_fuel += diff_fuel
        total_pairs_with_gain += 1

    gens_c = len(c["generations_history"])
    gens_d = len(d["generations_history"])

    llm_c = c["budget_consumed"]["llm_calls_consumed"]
    llm_d = d["budget_consumed"]["llm_calls_consumed"]

    print(f"\n--- PAIRE #{i+1} (Seed: {seed}) ---")
    print(f"Générations : C={gens_c}, D={gens_d} | Appels LLM : C={llm_c}, D={llm_d}")
    print(f"Groupe C (Essaim) : {cap_c['name']} (Gen {cap_c['generation']})")
    print(f"  Code: {cap_c['logic_code'][:100]}...")
    print(f"Groupe D (Mutation) : {cap_d['name']} (Gen {cap_d['generation']})")
    print(f"  Code: {cap_d['logic_code'][:100]}...")
    print(f"  In-Dist : Bacs C={bins_c} vs D={bins_d} | Fuel C={fuel_c} vs D={fuel_d} (Δ Fuel = -{diff_fuel} / {pct_fuel:.1f}%)")
    print(f"  OOD-1   : Bacs C={ood1_c} vs D={ood1_d} | Fuel C={ood1_fuel_c} vs D={ood1_fuel_d} (Δ Fuel = -{ood1_fuel_d - ood1_fuel_c})")
    print(f"  OOD-2   : Bacs C={ood2_c} vs D={ood2_d} | Fuel C={ood2_fuel_c} vs D={ood2_fuel_d} (Δ Fuel = -{ood2_fuel_d - ood2_fuel_c})")
    print(f"  Phylogénie C: {c.get('total_lineage_nodes', 0)} nœuds, {c.get('total_lineage_edges', 0)} transitions")
    print(f"  Phylogénie D: {d.get('total_lineage_nodes', 0)} nœuds, {d.get('total_lineage_edges', 0)} transitions")

print("\n" + "=" * 85)
print(f"Synthèse : {total_pairs_with_gain}/{len(data)} paires montrent une supériorité nette du Swarm Crossover sur le coût de calcul.")
print("=" * 85)

