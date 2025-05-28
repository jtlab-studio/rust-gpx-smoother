/// Simple Spike Removal - Remove GPS spikes while preserving 100% of terrain character
/// Perfect for routes where GPS data is already very accurate

pub fn simple_spike_removal_only(elevations: &[f64], _distances: &[f64]) -> Vec<f64> {
    remove_gps_spikes(elevations, 3.0)
}

pub fn remove_gps_spikes(elevations: &[f64], threshold_m: f64) -> Vec<f64> {
    if elevations.len() < 3 {
        return elevations.to_vec();
    }
    
    let original_gain = calculate_elevation_gain(elevations);
    let mut result = elevations.to_vec();
    let mut spikes_removed = 0;
    
    // 3-point spike detection: sudden jump up/down then back
    for i in 1..elevations.len() - 1 {
        let prev = elevations[i - 1];
        let curr = elevations[i];
        let next = elevations[i + 1];
        
        let up_change = curr - prev;
        let down_change = next - curr;
        
        // Spike detected: big change in opposite directions
        if up_change.abs() > threshold_m && 
           down_change.abs() > threshold_m && 
           up_change.signum() != down_change.signum() {
            
            // Replace spike with linear interpolation
            result[i] = (prev + next) / 2.0;
            spikes_removed += 1;
        }
    }
    
    let final_gain = calculate_elevation_gain(&result);
    
    if spikes_removed > 0 {
        println!("🔧 Spike removal: {} spikes removed, {:.1}m → {:.1}m ({:.1}% change)", 
                 spikes_removed, original_gain, final_gain, 
                 ((final_gain - original_gain) / original_gain) * 100.0);
    }
    
    result
}

fn calculate_elevation_gain(elevations: &[f64]) -> f64 {
    elevations.windows(2)
        .map(|w| if w[1] > w[0] { w[1] - w[0] } else { 0.0 })
        .sum()
}

pub fn calculate_simple_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
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
