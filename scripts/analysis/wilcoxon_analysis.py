#!/usr/bin/env python3
"""
NOMAD (DARWIN.RS) Scientific Analysis Script
Performs pre-registered statistical evaluation:
- Paired Wilcoxon Signed-Rank Test (unilateral: Group C vs Group D)
- Effect Size (Wilcoxon r and Cohen's d)
- In-Distribution (H1) and OOD Generalization (H2) validation
"""

import json
import math
import sys
from pathlib import Path


def compute_wilcoxon_signed_rank(group_c_scores, group_d_scores):
    """
    Computes paired Wilcoxon signed-rank test for Group C (Swarm) vs Group D (Mutation only).
    Note: We test whether Group C uses fewer bins than Group D (difference D - C > 0).
    """
    diffs = [d - c for c, d in zip(group_c_scores, group_d_scores)]
    mean_diff = sum(diffs) / len(diffs) if diffs else 0.0
    # Filter out zero differences
    nonzero_diffs = [d for d in diffs if d != 0]
    n = len(nonzero_diffs)
    if n == 0:
        return {
            "n_total": len(group_c_scores),
            "n_nonzero": 0,
            "w_plus": 0.0,
            "w_minus": 0.0,
            "z_stat": 0.0,
            "effect_size_r": 0.0,
            "p_value": 1.0,
            "mean_diff_d_minus_c": mean_diff,
            "status": "No difference observed",
        }

    # Rank absolute differences
    abs_diffs = sorted(enumerate(nonzero_diffs), key=lambda x: abs(x[1]))
    ranks = [0.0] * n

    # Handle ties with average ranks
    i = 0
    while i < n:
        j = i
        while j < n - 1 and abs(abs_diffs[j][1]) == abs(abs_diffs[j + 1][1]):
            j += 1
        avg_rank = (i + 1 + j + 1) / 2.0
        for k in range(i, j + 1):
            ranks[abs_diffs[k][0]] = avg_rank
        i = j + 1

    # Sum of ranks for positive differences (where Group C outperformed Group D)
    w_plus = sum(r for r, d in zip(ranks, nonzero_diffs) if d > 0)
    w_minus = sum(r for r, d in zip(ranks, nonzero_diffs) if d < 0)
    w = min(w_plus, w_minus)

    # Normal approximation for z-statistic
    mean_w = n * (n + 1) / 4.0
    var_w = n * (n + 1) * (2 * n + 1) / 24.0
    std_w = math.sqrt(var_w) if var_w > 0 else 1.0
    z = (w_plus - mean_w) / std_w

    # Effect size r = Z / sqrt(N)
    effect_r = z / math.sqrt(len(group_c_scores))

    # Standard normal CDF approximation (unilateral)
    # P(Z > z)
    p_value = 0.5 * math.erfc(z / math.sqrt(2))

    return {
        "n_total": len(group_c_scores),
        "n_nonzero": n,
        "w_plus": w_plus,
        "w_minus": w_minus,
        "z_stat": z,
        "effect_size_r": effect_r,
        "p_value": p_value,
        "mean_diff_d_minus_c": sum(diffs) / len(diffs),
    }


def analyze_experiment(report_file: str):
    with open(report_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    c_in_dist = []
    d_in_dist = []
    c_ood1 = []
    d_ood1 = []
    c_ood2 = []
    d_ood2 = []

    for report_c, report_d in data:
        c_in_dist.append(report_c["best_in_distribution_score"]["total_bins"])
        d_in_dist.append(report_d["best_in_distribution_score"]["total_bins"])

        c_ood1.append(report_c["best_ood1_score"]["total_bins"])
        d_ood1.append(report_d["best_ood1_score"]["total_bins"])

        c_ood2.append(report_c["best_ood2_score"]["total_bins"])
        d_ood2.append(report_d["best_ood2_score"]["total_bins"])

    print("\n=======================================================")
    print("      NOMAD (DARWIN.RS) SCIENTIFIC ANALYSIS REPORT     ")
    print("=======================================================")
    print(f"Total Paired Samples: {len(data)}")

    # 1. H1 Testing: In-Distribution Performance
    res_h1 = compute_wilcoxon_signed_rank(c_in_dist, d_in_dist)
    print("\n--- HYPOTHESIS H1 (In-Distribution Performance) ---")
    print(f"Group C Mean Bins: {sum(c_in_dist)/len(c_in_dist):.2f}")
    print(f"Group D Mean Bins: {sum(d_in_dist)/len(d_in_dist):.2f}")
    print(f"Mean Difference (D - C): {res_h1['mean_diff_d_minus_c']:+.2f} bins")
    print(f"Wilcoxon Z-stat: {res_h1['z_stat']:.4f}")
    print(f"Effect Size r:   {res_h1['effect_size_r']:.4f}")
    print(f"p-value:         {res_h1['p_value']:.5f}")
    h1_validated = res_h1["p_value"] < 0.05 and res_h1["effect_size_r"] > 0
    print(f"Status H1:       {'VALIDATED (p < 0.05)' if h1_validated else 'NOT SIGNIFICANT'}")

    # 2. H2 Testing: OOD Generalization
    res_ood1 = compute_wilcoxon_signed_rank(c_ood1, d_ood1)
    res_ood2 = compute_wilcoxon_signed_rank(c_ood2, d_ood2)
    print("\n--- HYPOTHESIS H2 (Out-Of-Distribution Generalization) ---")
    print("OOD-1 (Scale N=500):")
    print(f"  Group C Mean: {sum(c_ood1)/len(c_ood1):.2f} vs Group D: {sum(d_ood1)/len(d_ood1):.2f}")
    print(f"  p-value: {res_ood1['p_value']:.5f}, effect r: {res_ood1['effect_size_r']:.4f}")
    print("OOD-2 (Power-Law Shift N=500):")
    print(f"  Group C Mean: {sum(c_ood2)/len(c_ood2):.2f} vs Group D: {sum(d_ood2)/len(d_ood2):.2f}")
    print(f"  p-value: {res_ood2['p_value']:.5f}, effect r: {res_ood2['effect_size_r']:.4f}")
    print("=======================================================\n")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        analyze_experiment(sys.argv[1])
    else:
        print("Usage: python wilcoxon_analysis.py <path_to_comparison.json>")
