/// Simple Spike Removal - Remove GPS spikes while preserving 100% of terrain character
/// Updated to use enhanced spike removal from smart_spike_removal module

pub fn simple_spike_removal_only(elevations: &[f64], distances: &[f64]) -> Vec<f64> {
    crate::smart_spike_removal::simple_spike_removal_only(elevations, distances)
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
