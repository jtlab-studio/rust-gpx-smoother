/// Simple Universal GPX Smoother
pub fn universal_smooth(distances: &[f64], elevations: &[f64]) -> Vec<f64> {
    if distances.len() != elevations.len() || distances.len() < 5 {
        return elevations.to_vec();
    }
    
    // Adaptive window based on data size
    let window = if elevations.len() > 1000 { 45 } else { 25 };
    let mut result = Vec::with_capacity(elevations.len());
    
    for i in 0..elevations.len() {
        let start = if i >= window / 2 { i - window / 2 } else { 0 };
        let end = if i + window / 2 < elevations.len() { i + window / 2 } else { elevations.len() - 1 };
        
        let sum: f64 = elevations[start..=end].iter().sum();
        let count = end - start + 1;
        let smoothed = sum / count as f64;
        
        // Blend 80% smoothed, 20% original
        let blended = 0.8 * smoothed + 0.2 * elevations[i];
        result.push(blended);
    }
    
    // Apply bounds to prevent extreme changes
    let original_gain: f64 = elevations.windows(2)
        .map(|w| if w[1] > w[0] { w[1] - w[0] } else { 0.0 })
        .sum();
    
    let result_gain: f64 = result.windows(2)
        .map(|w| if w[1] > w[0] { w[1] - w[0] } else { 0.0 })
        .sum();
    
    if original_gain > 0.0 {
        let ratio = result_gain / original_gain;
        if ratio < 0.8 || ratio > 1.2 {
            // Scale to stay within 80%-120% bounds
            let target_ratio = ratio.max(0.8).min(1.2);
            let scale = target_ratio / ratio;
            
            let mut scaled = vec![result[0]];
            for i in 1..result.len() {
                let change = (result[i] - result[i-1]) * scale;
                scaled.push(scaled[i-1] + change);
            }
            return scaled;
        }
    }
    
    result
}

pub fn calculate_universal_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
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
