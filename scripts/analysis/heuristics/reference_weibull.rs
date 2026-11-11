pub fn score_bin(item_size: u32, bin_residual: u32, bin_capacity: u32, step_index: usize, total_items: usize, open_bins_count: usize, mean_residual: f64, next_item_size: u32, post_residual: u32, fitting_bins_count: usize) -> f64 {
    let r = post_residual as f64;
    let progress = step_index as f64 / total_items as f64;
    let mut score = r;
    if progress < 0.9 {
        if bin_residual == bin_capacity {
            score = 500.0;
        } else if post_residual <= 2 {
            score = r - 1000.0;
        } else if post_residual < 30 {
            score = r + 1000.0;
        }
    }
    score
}
