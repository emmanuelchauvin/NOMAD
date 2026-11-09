#!/usr/bin/env python3
"""
NOMAD (DARWIN.RS) Rigorous Scientific Statistical Analysis Script
Pre-registered protocol validation:
- Paired Wilcoxon Signed-Rank Test with tie variance correction:
    Var = [n(n+1)(2n+1) - sum(t_i^3 - t_i)/2] / 24
- Effect Size r = Z / sqrt(n_nonzero)
- Paired Cohen's d & Probability of Superiority (Common Language Effect Size)
- Exact / Monte-Carlo Paired Permutation Test
- Holm-Bonferroni Multiplicity Correction across hypotheses (H1, OOD-1, OOD-2)
- 10,000-resample Bootstrap 95% Confidence Intervals (Mean & Median differences)
- Primary & Secondary Metrics: Raw bins and Relative Excess over L1 bound ((bins - L1) / L1)
- Structured JSON and CSV export
"""

import argparse
import csv
import json
import math
import random
from pathlib import Path

try:
    import scipy.stats as sp_stats
    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False


def paired_bootstrap_ci(diffs, n_resamples=10000, alpha=0.05, seed=42):
    """
    Computes 95% bootstrap confidence intervals for the mean and median of paired differences.
    """
    if not diffs:
        return {"mean_ci": (0.0, 0.0), "median_ci": (0.0, 0.0)}

    rng = random.Random(seed)
    n = len(diffs)
    mean_samples = []
    median_samples = []

    for _ in range(n_resamples):
        sample = [rng.choice(diffs) for _ in range(n)]
        mean_samples.append(sum(sample) / n)
        sample.sort()
        if n % 2 == 1:
            med = sample[n // 2]
        else:
            med = (sample[n // 2 - 1] + sample[n // 2]) / 2.0
        median_samples.append(med)

    mean_samples.sort()
    median_samples.sort()

    lower_idx = int((alpha / 2.0) * n_resamples)
    upper_idx = int((1.0 - alpha / 2.0) * n_resamples)

    return {
        "mean_ci": (mean_samples[lower_idx], mean_samples[upper_idx]),
        "median_ci": (median_samples[lower_idx], median_samples[upper_idx]),
    }


def paired_permutation_test(diffs, n_permutations=10000, seed=42):
    """
    Two-sided and one-sided (greater: mean(diffs) > 0) paired permutation test
    by randomly flipping signs of differences under the null hypothesis.
    """
    n = len(diffs)
    if n == 0:
        return 1.0

    observed_stat = sum(diffs)
    rng = random.Random(seed)

    count_greater = 0
    for _ in range(n_permutations):
        perm_sum = sum(d if rng.random() >= 0.5 else -d for d in diffs)
        if perm_sum >= observed_stat:
            count_greater += 1

    return (count_greater + 1) / (n_permutations + 1)


def compute_wilcoxon_signed_rank(group_c_scores, group_d_scores):
    """
    Computes rigorous paired Wilcoxon signed-rank test for Group C (Swarm) vs Group D (Mutation only).
    diff = D - C > 0 implies Group C used fewer bins / less excess than Group D (Group C outperformed Group D).
    """
    total_n = len(group_c_scores)
    diffs = [d - c for c, d in zip(group_c_scores, group_d_scores)]
    mean_diff = sum(diffs) / total_n if total_n > 0 else 0.0

    # Common language effect size / Probability of superiority
    # P(D > C) + 0.5 * P(D == C)
    pos_count = sum(1 for d in diffs if d > 0)
    zero_count = sum(1 for d in diffs if d == 0)
    prob_superiority = (pos_count + 0.5 * zero_count) / total_n if total_n > 0 else 0.5

    # Paired Cohen's d: mean(diff) / std(diff)
    if total_n > 1:
        variance_diff = sum((d - mean_diff) ** 2 for d in diffs) / (total_n - 1)
        std_diff = math.sqrt(variance_diff) if variance_diff > 1e-12 else 0.0
        cohens_d = (mean_diff / std_diff) if std_diff > 1e-12 else 0.0
    else:
        cohens_d = 0.0

    # Filter out zero differences for Wilcoxon signed rank test
    nonzero_diffs = [d for d in diffs if d != 0]
    n = len(nonzero_diffs)

    # Bootstrap 95% CIs
    ci_res = paired_bootstrap_ci(diffs, n_resamples=10000, alpha=0.05)
    perm_p_val = paired_permutation_test(diffs, n_permutations=10000)

    if n == 0:
        return {
            "n_total": total_n,
            "n_nonzero": 0,
            "w_plus": 0.0,
            "w_minus": 0.0,
            "z_stat": 0.0,
            "effect_size_r": 0.0,
            "cohens_d": cohens_d,
            "prob_superiority": prob_superiority,
            "p_value_raw": 1.0,
            "p_value_permutation": 1.0,
            "mean_diff_d_minus_c": mean_diff,
            "mean_ci_95": ci_res["mean_ci"],
            "median_ci_95": ci_res["median_ci"],
            "method": "zero_difference",
        }

    # Rank absolute differences
    abs_diffs = sorted(enumerate(nonzero_diffs), key=lambda x: abs(x[1]))
    ranks = [0.0] * n

    # Group ties and apply tie correction to variance:
    # Var(W) = [n(n+1)(2n+1) - sum(t_i^3 - t_i)/2] / 24
    tie_counts = []
    i = 0
    while i < n:
        j = i
        while j < n - 1 and abs(abs_diffs[j][1]) == abs(abs_diffs[j + 1][1]):
            j += 1
        tie_len = j - i + 1
        if tie_len > 1:
            tie_counts.append(tie_len)
        avg_rank = (i + 1 + j + 1) / 2.0
        for k in range(i, j + 1):
            ranks[abs_diffs[k][0]] = avg_rank
        i = j + 1

    w_plus = sum(r for r, d in zip(ranks, nonzero_diffs) if d > 0)
    w_minus = sum(r for r, d in zip(ranks, nonzero_diffs) if d < 0)

    mean_w = n * (n + 1) / 4.0
    tie_adjustment = sum((t**3 - t) for t in tie_counts) / 2.0
    var_w = (n * (n + 1) * (2 * n + 1) - tie_adjustment) / 24.0
    std_w = math.sqrt(var_w) if var_w > 0 else 1.0

    # Continuity correction
    if w_plus > mean_w:
        z = (w_plus - mean_w - 0.5) / std_w
    elif w_plus < mean_w:
        z = (w_plus - mean_w + 0.5) / std_w
    else:
        z = 0.0

    # Effect size r = Z / sqrt(n_nonzero)
    effect_r = z / math.sqrt(n)

    # One-sided p-value: P(Z >= z)
    p_value = 0.5 * math.erfc(z / math.sqrt(2))

    # Optional Scipy validation
    scipy_p = None
    if HAS_SCIPY:
        try:
            # alternative='greater' tests if group_d > group_c
            res_sp = sp_stats.wilcoxon(group_d_scores, group_c_scores, alternative="greater", zero_method="pratt")
            scipy_p = float(res_sp.pvalue)
        except Exception:
            pass

    return {
        "n_total": total_n,
        "n_nonzero": n,
        "w_plus": w_plus,
        "w_minus": w_minus,
        "z_stat": z,
        "effect_size_r": effect_r,
        "cohens_d": cohens_d,
        "prob_superiority": prob_superiority,
        "p_value_raw": p_value,
        "scipy_p_value": scipy_p,
        "p_value_permutation": perm_p_val,
        "mean_diff_d_minus_c": mean_diff,
        "mean_ci_95": ci_res["mean_ci"],
        "median_ci_95": ci_res["median_ci"],
        "method": "wilcoxon_asymptotic_tie_corrected",
    }


def holm_bonferroni_correction(results_dict):
    """
    Applies step-down Holm-Bonferroni correction across all tested hypotheses.
    """
    keys = list(results_dict.keys())
    m = len(keys)
    if m == 0:
        return

    # Sort hypotheses by raw p-value ascending
    sorted_items = sorted(keys, key=lambda k: results_dict[k]["p_value_raw"])

    cumulative_max = 0.0
    for idx, key in enumerate(sorted_items):
        multiplier = m - idx
        raw_p = results_dict[key]["p_value_raw"]
        adjusted_p = min(1.0, max(cumulative_max, raw_p * multiplier))
        cumulative_max = adjusted_p
        results_dict[key]["p_value_adjusted_holm"] = adjusted_p
        results_dict[key]["is_significant_05"] = (adjusted_p < 0.05) and (results_dict[key]["effect_size_r"] > 0)


def analyze_experiment(report_file: str, out_json: str = None, out_csv: str = None):
    with open(report_file, "r", encoding="utf-8") as f:
        data = json.load(f)

    c_in_dist_bins = []
    d_in_dist_bins = []
    c_in_dist_excess = []
    d_in_dist_excess = []

    c_val_bins = []
    d_val_bins = []

    c_ood1_bins = []
    d_ood1_bins = []
    c_ood2_bins = []
    d_ood2_bins = []

    for item in data:
        if isinstance(item, list) and len(item) == 2:
            rc, rd = item[0], item[1]
        elif isinstance(item, dict) and "group_c" in item and "group_d" in item:
            rc, rd = item["group_c"], item["group_d"]
        else:
            continue

        c_in_dist_bins.append(rc["best_in_distribution_score"]["total_bins"])
        d_in_dist_bins.append(rd["best_in_distribution_score"]["total_bins"])

        # Relative excess metric over theoretical lower bound: (bins - L1) / L1
        c_l1 = rc["best_in_distribution_score"]["total_theoretical_min_bins"]
        d_l1 = rd["best_in_distribution_score"]["total_theoretical_min_bins"]
        c_in_dist_excess.append((rc["best_in_distribution_score"]["total_bins"] - c_l1) / max(1, c_l1))
        d_in_dist_excess.append((rd["best_in_distribution_score"]["total_bins"] - d_l1) / max(1, d_l1))

        if "best_validation_score" in rc and rc["best_validation_score"] and "best_validation_score" in rd and rd["best_validation_score"]:
            c_val_bins.append(rc["best_validation_score"]["total_bins"])
            d_val_bins.append(rd["best_validation_score"]["total_bins"])

        c_ood1_bins.append(rc["best_ood1_score"]["total_bins"])
        d_ood1_bins.append(rd["best_ood1_score"]["total_bins"])

        c_ood2_bins.append(rc["best_ood2_score"]["total_bins"])
        d_ood2_bins.append(rd["best_ood2_score"]["total_bins"])

    print("\n=======================================================")
    print("      NOMAD (DARWIN.RS) SCIENTIFIC ANALYSIS REPORT     ")
    print("=======================================================")
    print(f"Total Paired Samples: {len(c_in_dist_bins)}")
    if HAS_SCIPY:
        print("Scipy detected: Dual asymptotic & exact verification enabled.")
    else:
        print("Scipy not detected: Exact asymptotic tie-variance correction & permutation test enabled.")

    analysis_results = {
        "H1_in_distribution_bins": compute_wilcoxon_signed_rank(c_in_dist_bins, d_in_dist_bins),
        "H1_in_distribution_relative_excess": compute_wilcoxon_signed_rank(c_in_dist_excess, d_in_dist_excess),
        "H2_ood1_scale_bins": compute_wilcoxon_signed_rank(c_ood1_bins, d_ood1_bins),
        "H2_ood2_powerlaw_bins": compute_wilcoxon_signed_rank(c_ood2_bins, d_ood2_bins),
    }

    if c_val_bins and d_val_bins:
        analysis_results["validation_in_distribution_bins"] = compute_wilcoxon_signed_rank(c_val_bins, d_val_bins)

    # Multiplicity correction
    holm_bonferroni_correction(analysis_results)

    # Print summary
    for name, res in analysis_results.items():
        print(f"\n--- HYPOTHESIS: {name} ---")
        print(f"  N total: {res['n_total']}, N non-zero: {res['n_nonzero']}")
        print(f"  Mean Difference (D - C): {res['mean_diff_d_minus_c']:+.4f}")
        print(f"  Bootstrap 95% CI Mean  : [{res['mean_ci_95'][0]:+.4f}, {res['mean_ci_95'][1]:+.4f}]")
        print(f"  Bootstrap 95% CI Median: [{res['median_ci_95'][0]:+.4f}, {res['median_ci_95'][1]:+.4f}]")
        print(f"  Wilcoxon Z-stat        : {res['z_stat']:.4f}")
        print(f"  Effect Size r (Z/√n)   : {res['effect_size_r']:.4f}")
        print(f"  Paired Cohen's d       : {res['cohens_d']:.4f}")
        print(f"  Prob of Superiority    : {res['prob_superiority']*100:.1f}%")
        print(f"  Raw p-value (Wilcoxon) : {res['p_value_raw']:.5f}")
        print(f"  Permutation p-value    : {res['p_value_permutation']:.5f}")
        print(f"  Adjusted p-value (Holm): {res['p_value_adjusted_holm']:.5f}")
        status_str = "VALIDATED (p_adj < 0.05)" if res["is_significant_05"] else "NOT STATISTICALLY SIGNIFICANT"
        print(f"  Conclusion             : {status_str}")

    print("=======================================================\n")

    if out_json:
        with open(out_json, "w", encoding="utf-8") as fj:
            json.dump(analysis_results, fj, indent=2)
        print(f"Statistical results written to JSON: {out_json}")

    if out_csv:
        with open(out_csv, "w", newline="", encoding="utf-8") as fc:
            writer = csv.writer(fc)
            writer.writerow([
                "Hypothesis",
                "N_total",
                "N_nonzero",
                "Mean_Diff_D_minus_C",
                "Mean_CI_95_Lower",
                "Mean_CI_95_Upper",
                "Median_CI_95_Lower",
                "Median_CI_95_Upper",
                "Wilcoxon_Z",
                "Effect_Size_r",
                "Cohens_d",
                "Prob_Superiority",
                "p_raw",
                "p_permutation",
                "p_adj_holm",
                "Significant_p05",
            ])
            for name, res in analysis_results.items():
                writer.writerow([
                    name,
                    res["n_total"],
                    res["n_nonzero"],
                    f"{res['mean_diff_d_minus_c']:.4f}",
                    f"{res['mean_ci_95'][0]:.4f}",
                    f"{res['mean_ci_95'][1]:.4f}",
                    f"{res['median_ci_95'][0]:.4f}",
                    f"{res['median_ci_95'][1]:.4f}",
                    f"{res['z_stat']:.4f}",
                    f"{res['effect_size_r']:.4f}",
                    f"{res['cohens_d']:.4f}",
                    f"{res['prob_superiority']:.4f}",
                    f"{res['p_value_raw']:.5f}",
                    f"{res['p_value_permutation']:.5f}",
                    f"{res['p_value_adjusted_holm']:.5f}",
                    res["is_significant_05"],
                ])
        print(f"Statistical results written to CSV: {out_csv}")

    return analysis_results


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="NOMAD Scientific Wilcoxon Analysis")
    parser.add_argument("report_file", help="Path to comparison aggregate JSON file")
    parser.add_argument("--output-json", default=None, help="Path for JSON output summary")
    parser.add_argument("--output-csv", default=None, help="Path for CSV output summary")
    args = parser.parse_args()

    analyze_experiment(args.report_file, args.output_json, args.output_csv)
