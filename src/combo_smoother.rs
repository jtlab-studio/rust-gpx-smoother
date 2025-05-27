/// Combo Smoothing Algorithm
/// Combines outlier detection, adaptive windowing, Savitzky-Golay-like filtering, and gradient capping
use std::f64;

/// Given parallel slices of cumulative distance `d` and elevation `e`,
/// returns a new Vec of smoothed elevations.
pub fn universal_smooth(d: &[f64], e: &[f64]) -> Vec<f64> {
    let n = d.len();
    assert_eq!(n, e.len(), "distance & elevation must be same length");
    if n < 5 {
        // Too few points to smooth
        return e.to_vec();
    }
    
    // 1) Outlier detection via MAD (Median Absolute Deviation) on gradients
    let grads: Vec<f64> = d.windows(2)
        .zip(e.windows(2))
        .map(|(dd, ee)| {
            let dx = dd[1] - dd[0];
            if dx.abs() < 1e-10 {
                0.0 // avoid division by zero
            } else {
                (ee[1] - ee[0]) / dx
            }
        })
        .collect();
    
    let median_grad = median(&grads);
    let mad = median(&grads.iter()
        .map(|g| (g - median_grad).abs())
        .collect::<Vec<_>>());
    
    let threshold = 3.0 * mad;
    
    // Build a masked elevation array, linearly interpolating over outliers
    let mut e_interp = e.to_vec();
    let mut bad_idx = Vec::new();
    
    for (i, &g) in grads.iter().enumerate() {
        if (g - median_grad).abs() > threshold {
            // mark the later point as bad
            bad_idx.push(i + 1);
        }
    }
    
    // interpolate: for each bad index, find nearest good neighbors
    for &i in &bad_idx {
        // find prev good
        let lo = (0..i).rev().find(|&j| !bad_idx.contains(&j)).unwrap_or(0);
        let hi = (i + 1..n).find(|&j| !bad_idx.contains(&j)).unwrap_or(n - 1);
        
        if lo != hi {
            let t = (d[i] - d[lo]) / (d[hi] - d[lo]);
            e_interp[i] = e_interp[lo] + t * (e_interp[hi] - e_interp[lo]);
        }
    }
    
    // 2) Adaptive window size calculation
    let diffs_e: Vec<f64> = e_interp.windows(2)
        .map(|w| w[1] - w[0])
        .collect();
    
    let sigma = std_dev(&diffs_e);
    let mu_d = d.windows(2).map(|w| w[1] - w[0]).sum::<f64>() / (n as f64 - 1.0);
    
    // tuning constants
    let alpha = 50.0;
    let mut window = (alpha * (sigma / mu_d.max(1e-10))).round() as usize;
    
    // bounds & make odd
    window = window.clamp(51, 301);
    if window % 2 == 0 { 
        window += 1; 
    }
    
    // 3) Apply polynomial smoothing (simplified Savitzky-Golay approach)
    let e_smooth = polynomial_smooth(&e_interp, window, 3);
    
    // 4) Gradient capping & reintegration
    let capped_grads: Vec<f64> = d.windows(2)
        .zip(e_smooth.windows(2))
        .map(|(dd, ee)| {
            let dx = dd[1] - dd[0];
            if dx.abs() < 1e-10 {
                0.0
            } else {
                let raw = (ee[1] - ee[0]) / dx;
                raw.clamp(-0.5, 0.6)  // cap between -50% and +60%
            }
        })
        .collect();
    
    let mut e_final = Vec::with_capacity(n);
    e_final.push(e_smooth[0]);
    
    for (i, &g) in capped_grads.iter().enumerate() {
        let dist_delta = d[i + 1] - d[i];
        e_final.push(e_final[i] + g * dist_delta);
    }
    
    e_final
}

/// Simplified polynomial smoothing (approximates Savitzky-Golay)
fn polynomial_smooth(data: &[f64], window: usize, _order: usize) -> Vec<f64> {
    let n = data.len();
    let mut result = Vec::with_capacity(n);
    
    for i in 0..n {
        let start = if i >= window / 2 { i - window / 2 } else { 0 };
        let end = if i + window / 2 < n { i + window / 2 } else { n - 1 };
        
        // For simplicity, use weighted moving average with Gaussian-like weights
        let center = (start + end) / 2;
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        
        for j in start..=end {
            let distance = (j as f64 - center as f64).abs();
            let sigma = window as f64 / 6.0;
            let weight = (-0.5 * (distance / sigma).powi(2)).exp();
            
            weighted_sum += data[j] * weight;
            weight_sum += weight;
        }
        
        result.push(weighted_sum / weight_sum);
    }
    
    result
}

/// Calculate median of a vector
fn median(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let n = sorted.len();
    if n % 2 == 0 {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    }
}

/// Calculate standard deviation
fn std_dev(data: &[f64]) -> f64 {
    if data.len() <= 1 {
        return 0.0;
    }
    
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let variance = data.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / (data.len() - 1) as f64;
    
    variance.sqrt()
}

pub fn calculate_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
    let mut gain = 0.0;
    let mut loss = 0.0;
    
    for w in elevations.windows(2) {
        let delta = w[1] - w[0];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += -delta;
        }
    }
    
    (gain, loss)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_universal_smooth() {
        let distances = vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0];
        let elevations = vec![100.0, 102.0, 105.0, 103.0, 107.0, 110.0];
        
        let smoothed = universal_smooth(&distances, &elevations);
        
        assert_eq!(smoothed.len(), elevations.len());
        // Smoothed values should be within reasonable range
        for &val in &smoothed {
            assert!(val >= 95.0 && val <= 115.0);
        }
    }
    
    #[test]
    fn test_median() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[]), 0.0);
    }
    
    #[test]
    fn test_std_dev() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let result = std_dev(&data);
        assert!((result - 2.138).abs() < 0.01); // approximately 2.138
    }
}
