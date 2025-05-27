/// Enhanced Combo Smoothing Algorithm with configurable parameters
/// Provides more control over smoothing aggressiveness and preservation of original data
use std::f64;

#[derive(Debug, Clone)]
pub struct EnhancedComboConfig {
    /// Window sizing factor (lower = less aggressive smoothing)
    pub alpha: f64,
    /// Minimum and maximum window sizes
    pub window_min: usize,
    pub window_max: usize,
    /// Outlier detection threshold multiplier (higher = less sensitive)
    pub outlier_threshold_multiplier: f64,
    /// Gradient clamping limits
    pub gradient_min: f64,
    pub gradient_max: f64,
    /// Blending factor between raw and smoothed gradients (0.0 = all smoothed, 1.0 = all raw)
    pub blend_factor: f64,
    /// Enable/disable gradient capping
    pub enable_gradient_capping: bool,
    /// Enable/disable gradient blending
    pub enable_gradient_blending: bool,
}

impl Default for EnhancedComboConfig {
    fn default() -> Self {
        EnhancedComboConfig {
            alpha: 50.0,
            window_min: 51,
            window_max: 301,
            outlier_threshold_multiplier: 3.0,
            gradient_min: -0.5,
            gradient_max: 0.6,
            blend_factor: 0.0,
            enable_gradient_capping: true,
            enable_gradient_blending: false,
        }
    }
}

impl EnhancedComboConfig {
    /// Conservative smoothing - preserves more of the original data
    pub fn conservative() -> Self {
        EnhancedComboConfig {
            alpha: 20.0,
            window_min: 21,
            window_max: 101,
            outlier_threshold_multiplier: 5.0,
            gradient_min: -1.0,
            gradient_max: 1.0,
            blend_factor: 0.2,
            enable_gradient_capping: true,
            enable_gradient_blending: true,
            ..Default::default()
        }
    }
    
    /// Moderate smoothing - balanced approach
    pub fn moderate() -> Self {
        EnhancedComboConfig {
            alpha: 35.0,
            window_min: 31,
            window_max: 151,
            outlier_threshold_multiplier: 4.0,
            gradient_min: -0.7,
            gradient_max: 0.8,
            blend_factor: 0.15,
            enable_gradient_capping: true,
            enable_gradient_blending: true,
            ..Default::default()
        }
    }
    
    /// Aggressive smoothing - original combo smoother behavior
    pub fn aggressive() -> Self {
        Self::default()
    }
    
    /// Experimental - no gradient capping, heavy blending
    pub fn experimental() -> Self {
        EnhancedComboConfig {
            alpha: 15.0,
            window_min: 15,
            window_max: 81,
            outlier_threshold_multiplier: 7.0,
            gradient_min: -2.0,
            gradient_max: 2.0,
            blend_factor: 0.3,
            enable_gradient_capping: false,
            enable_gradient_blending: true,
            ..Default::default()
        }
    }
}

/// Enhanced version of universal_smooth with configurable parameters
pub fn enhanced_universal_smooth(d: &[f64], e: &[f64], config: &EnhancedComboConfig) -> Vec<f64> {
    let n = d.len();
    assert_eq!(n, e.len(), "distance & elevation must be same length");
    if n < 5 {
        return e.to_vec();
    }
    
    // 1) Outlier detection via MAD on gradients
    let raw_grads: Vec<f64> = d.windows(2)
        .zip(e.windows(2))
        .map(|(dd, ee)| {
            let dx = dd[1] - dd[0];
            if dx.abs() < 1e-10 {
                0.0
            } else {
                (ee[1] - ee[0]) / dx
            }
        })
        .collect();
    
    let median_grad = median(&raw_grads);
    let mad = median(&raw_grads.iter()
        .map(|g| (g - median_grad).abs())
        .collect::<Vec<_>>());
    
    // Enhanced: Configurable outlier threshold
    let threshold = config.outlier_threshold_multiplier * mad;
    
    // Build a masked elevation array, linearly interpolating over outliers
    let mut e_interp = e.to_vec();
    let mut bad_idx = Vec::new();
    
    for (i, &g) in raw_grads.iter().enumerate() {
        if (g - median_grad).abs() > threshold {
            bad_idx.push(i + 1);
        }
    }
    
    // Interpolate over outliers
    for &i in &bad_idx {
        let lo = (0..i).rev().find(|&j| !bad_idx.contains(&j)).unwrap_or(0);
        let hi = (i + 1..n).find(|&j| !bad_idx.contains(&j)).unwrap_or(n - 1);
        
        if lo != hi {
            let t = (d[i] - d[lo]) / (d[hi] - d[lo]);
            e_interp[i] = e_interp[lo] + t * (e_interp[hi] - e_interp[lo]);
        }
    }
    
    // 2) Enhanced: Configurable adaptive window sizing
    let diffs_e: Vec<f64> = e_interp.windows(2)
        .map(|w| w[1] - w[0])
        .collect();
    
    let sigma = std_dev(&diffs_e);
    let mu_d = d.windows(2).map(|w| w[1] - w[0]).sum::<f64>() / (n as f64 - 1.0);
    
    let mut window = (config.alpha * (sigma / mu_d.max(1e-10))).round() as usize;
    window = window.clamp(config.window_min, config.window_max);
    if window % 2 == 0 { 
        window += 1; 
    }
    
    // 3) Apply polynomial smoothing
    let e_smooth = polynomial_smooth(&e_interp, window, 3);
    
    // 4) Calculate smoothed gradients
    let smooth_grads: Vec<f64> = d.windows(2)
        .zip(e_smooth.windows(2))
        .map(|(dd, ee)| {
            let dx = dd[1] - dd[0];
            if dx.abs() < 1e-10 {
                0.0
            } else {
                (ee[1] - ee[0]) / dx
            }
        })
        .collect();
    
    // 5) Enhanced: Optional gradient blending
    let blended_grads: Vec<f64> = if config.enable_gradient_blending {
        raw_grads.iter()
            .zip(smooth_grads.iter())
            .map(|(&raw, &smooth)| {
                config.blend_factor * raw + (1.0 - config.blend_factor) * smooth
            })
            .collect()
    } else {
        smooth_grads
    };
    
    // 6) Enhanced: Configurable gradient capping
    let final_grads: Vec<f64> = if config.enable_gradient_capping {
        blended_grads.iter()
            .map(|&g| g.clamp(config.gradient_min, config.gradient_max))
            .collect()
    } else {
        blended_grads
    };
    
    // 7) Reintegrate elevations
    let mut e_final = Vec::with_capacity(n);
    e_final.push(e_smooth[0]);
    
    for (i, &g) in final_grads.iter().enumerate() {
        let dist_delta = d[i + 1] - d[i];
        e_final.push(e_final[i] + g * dist_delta);
    }
    
    e_final
}

/// Convenience function using conservative settings
pub fn enhanced_universal_smooth_conservative(d: &[f64], e: &[f64]) -> Vec<f64> {
    enhanced_universal_smooth(d, e, &EnhancedComboConfig::conservative())
}

/// Convenience function using moderate settings
pub fn enhanced_universal_smooth_moderate(d: &[f64], e: &[f64]) -> Vec<f64> {
    enhanced_universal_smooth(d, e, &EnhancedComboConfig::moderate())
}

/// Convenience function using experimental settings
pub fn enhanced_universal_smooth_experimental(d: &[f64], e: &[f64]) -> Vec<f64> {
    enhanced_universal_smooth(d, e, &EnhancedComboConfig::experimental())
}

/// Simplified polynomial smoothing (same as combo_smoother)
fn polynomial_smooth(data: &[f64], window: usize, _order: usize) -> Vec<f64> {
    let n = data.len();
    let mut result = Vec::with_capacity(n);
    
    for i in 0..n {
        let start = if i >= window / 2 { i - window / 2 } else { 0 };
        let end = if i + window / 2 < n { i + window / 2 } else { n - 1 };
        
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

pub fn calculate_elevation_gain_loss_enhanced(elevations: &[f64]) -> (f64, f64) {
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
