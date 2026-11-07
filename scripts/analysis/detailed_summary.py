import json

with open("comparison_ollama.json", encoding="utf-8") as f:
    data = json.load(f)

print("=" * 90)
print("  ANALYSE MULTI-DATASET : ENTRAÎNEMENT, OOD-1 (ÉCHELLE N=500), OOD-2 (LOI DE PUISSANCE)")
print("=" * 90)

for i, (c, d) in enumerate(data):
    bins_c = c["best_in_distribution_score"]["total_bins"]
    bins_d = d["best_in_distribution_score"]["total_bins"]
    fuel_c = c["best_in_distribution_score"]["total_fuel"]
    fuel_d = d["best_in_distribution_score"]["total_fuel"]

    ood1_bins_c = c["best_ood1_score"]["total_bins"]
    ood1_bins_d = d["best_ood1_score"]["total_bins"]
    ood1_fuel_c = c["best_ood1_score"]["total_fuel"]
    ood1_fuel_d = d["best_ood1_score"]["total_fuel"]

    ood2_bins_c = c["best_ood2_score"]["total_bins"]
    ood2_bins_d = d["best_ood2_score"]["total_bins"]
    ood2_fuel_c = c["best_ood2_score"]["total_fuel"]
    ood2_fuel_d = d["best_ood2_score"]["total_fuel"]

    print(f"\n--- PAIRE #{i+1} (Seed: {c['rng_seed']}) ---")
    print(f"Meilleure Capsule C: {c['best_capsule']['name']} ({c['best_capsule']['semantic_interface']})")
    print(f"Meilleure Capsule D: {d['best_capsule']['name']} ({d['best_capsule']['semantic_interface']})")
    print(f"  In-Dist : Bacs C={bins_c} vs D={bins_d} | Fuel C={fuel_c} vs D={fuel_d} (diff: {fuel_c - fuel_d:+d})")
    print(f"  OOD-1   : Bacs C={ood1_bins_c} vs D={ood1_bins_d} | Fuel C={ood1_fuel_c} vs D={ood1_fuel_d} (diff: {ood1_fuel_c - ood1_fuel_d:+d})")
    print(f"  OOD-2   : Bacs C={ood2_bins_c} vs D={ood2_bins_d} | Fuel C={ood2_fuel_c} vs D={ood2_fuel_d} (diff: {ood2_fuel_c - ood2_fuel_d:+d})")
