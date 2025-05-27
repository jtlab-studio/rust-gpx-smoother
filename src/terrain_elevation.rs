/// Simplified Terrain-based elevation correction - 100% terrain data
/// Three options: Raw, 150m smoothing, 300m smoothing
/// Uses zoom level 15 with disk caching and rate limiting (max 3000 requests/second)
use reqwest::blocking::get;
use image::io::Reader as ImgReader;
use image::{DynamicImage, ImageFormat, GenericImageView};
use std::io::Cursor;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::time::{Duration, Instant};
use std::thread;

#[derive(Debug, Clone)]
pub struct TerrainConfig {
    /// Fixed zoom level for maximum accuracy
    pub zoom_level: u8,
    /// Cache directory path
    pub cache_dir: PathBuf,
    /// Rate limiting: max requests per second
    pub max_requests_per_second: u32,
    /// Rolling smoothing window in meters (0.0 = no smoothing)
    pub smoothing_window_meters: f64,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        let cache_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("terrain_cache");
        
        TerrainConfig {
            zoom_level: 15,  // Maximum accuracy zoom level
            cache_dir,
            max_requests_per_second: 3000,
            smoothing_window_meters: 150.0,  // Default 150m smoothing
        }
    }
}

impl TerrainConfig {
    /// Raw terrain data - no smoothing
    pub fn raw() -> Self {
        TerrainConfig {
            smoothing_window_meters: 0.0,  // No smoothing
            ..Default::default()
        }
    }
    
    /// 150 meter smoothing window
    pub fn smooth_150m() -> Self {
        TerrainConfig {
            smoothing_window_meters: 150.0,
            ..Default::default()
        }
    }
    
    /// 300 meter smoothing window
    pub fn smooth_300m() -> Self {
        TerrainConfig {
            smoothing_window_meters: 300.0,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TileKey {
    z: u8,
    x: u32,
    y: u32,
}

impl TileKey {
    fn cache_filename(&self) -> String {
        format!("tile_z{}_x{}_y{}.png", self.z, self.x, self.y)
    }
}

/// Optimized disk-based tile cache with rate limiting
struct OptimizedTileCache {
    cache_dir: PathBuf,
    memory_cache: HashMap<TileKey, DynamicImage>,
    last_request_time: Instant,
    min_request_interval: Duration,
    request_count: u32,
    request_window_start: Instant,
}

impl OptimizedTileCache {
    fn new(config: &TerrainConfig) -> Result<Self> {
        // Create cache directory if it doesn't exist
        fs::create_dir_all(&config.cache_dir)?;
        
        let min_request_interval = Duration::from_nanos(1_000_000_000 / config.max_requests_per_second as u64);
        
        Ok(OptimizedTileCache {
            cache_dir: config.cache_dir.clone(),
            memory_cache: HashMap::new(),
            last_request_time: Instant::now() - Duration::from_secs(1),
            min_request_interval,
            request_count: 0,
            request_window_start: Instant::now(),
        })
    }
    
    fn get_tile(&mut self, tile_key: TileKey) -> Result<DynamicImage> {
        // Check memory cache first
        if let Some(img) = self.memory_cache.get(&tile_key) {
            return Ok(img.clone());
        }
        
        // Check disk cache
        let cache_path = self.cache_dir.join(tile_key.cache_filename());
        if cache_path.exists() {
            match image::open(&cache_path) {
                Ok(img) => {
                    // Store in memory cache for faster access
                    self.memory_cache.insert(tile_key, img.clone());
                    return Ok(img);
                },
                Err(_) => {
                    // Corrupted cache file, delete it
                    let _ = fs::remove_file(&cache_path);
                }
            }
        }
        
        // Need to fetch from network - apply rate limiting
        self.apply_rate_limiting();
        
        // Fetch tile from Terrarium
        let url = format!(
            "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{}/{}/{}.png",
            tile_key.z, tile_key.x, tile_key.y
        );
        
        println!("Fetching tile: z={}, x={}, y={}", tile_key.z, tile_key.x, tile_key.y);
        
        let resp = get(&url)?.bytes()?;
        let img = ImgReader::new(Cursor::new(&resp))
            .with_guessed_format()?
            .decode()?;
        
        // Save to disk cache
        if let Err(e) = img.save_with_format(&cache_path, ImageFormat::Png) {
            eprintln!("Warning: Failed to save tile to cache: {}", e);
        }
        
        // Store in memory cache
        self.memory_cache.insert(tile_key, img.clone());
        
        Ok(img)
    }
    
    fn apply_rate_limiting(&mut self) {
        let now = Instant::now();
        
        // Reset counter every second
        if now.duration_since(self.request_window_start) >= Duration::from_secs(1) {
            self.request_count = 0;
            self.request_window_start = now;
        }
        
        // Check if we need to wait
        let time_since_last = now.duration_since(self.last_request_time);
        if time_since_last < self.min_request_interval {
            let sleep_duration = self.min_request_interval - time_since_last;
            thread::sleep(sleep_duration);
        }
        
        self.last_request_time = Instant::now();
        self.request_count += 1;
        
        if self.request_count % 100 == 0 {
            println!("Processed {} tile requests", self.request_count);
        }
    }
}

/// Convert lat/lon to tile coordinates at zoom level 15
fn latlon_to_tile_coords(lat: f64, lon: f64, zoom: u8) -> (u32, u32, u32, u32) {
    let n = 2_f64.powi(zoom as i32);
    let lat_rad = lat.to_radians();
    
    let xtile = ((lon + 180.0) / 360.0 * n) as u32;
    let ytile = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n) as u32;
    
    // Calculate pixel position within 256x256 tile
    let tile_size = 256.0;
    let x_frac = (lon + 180.0) / 360.0 * n - xtile as f64;
    let y_frac = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n - ytile as f64;
    
    let xpixel = (x_frac * tile_size) as u32;
    let ypixel = (y_frac * tile_size) as u32;
    
    (xtile, ytile, xpixel.min(255), ypixel.min(255))
}

/// Extract elevation from Terrarium tile at specific pixel
fn extract_terrarium_elevation(img: &DynamicImage, x: u32, y: u32) -> f64 {
    let pixel = img.get_pixel(x, y);
    let rgba = pixel.0;
    let (r, g, b) = (rgba[0] as u32, rgba[1] as u32, rgba[2] as u32);
    
    // Terrarium formula: elevation = (R * 256 + G + B/256) - 32768
    (r * 256 + g) as f64 + (b as f64 / 256.0) - 32768.0
}

/// Get 100% terrain elevations for all coordinates
pub fn get_terrain_elevations(
    coordinates: &[(f64, f64, f64)], // (lat, lon, gps_elevation) - GPS elevation ignored
    config: &TerrainConfig
) -> Vec<f64> {
    let mut cache = match OptimizedTileCache::new(config) {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("Failed to initialize tile cache: {}. Cannot proceed without terrain data.", e);
            return vec![];
        }
    };
    
    let mut terrain_elevations = Vec::with_capacity(coordinates.len());
    let mut unique_tiles_needed = std::collections::HashSet::new();
    
    // First pass: identify all unique tiles needed
    for &(lat, lon, _) in coordinates {
        let (xtile, ytile, _, _) = latlon_to_tile_coords(lat, lon, config.zoom_level);
        unique_tiles_needed.insert(TileKey { z: config.zoom_level, x: xtile, y: ytile });
    }
    
    println!("Processing {} coordinates across {} unique tiles (zoom {})", 
             coordinates.len(), unique_tiles_needed.len(), config.zoom_level);
    
    // Second pass: get terrain elevation for each coordinate
    for (i, &(lat, lon, _gps_elev)) in coordinates.iter().enumerate() {
        if i % 500 == 0 {
            println!("Processing coordinate {} of {}", i, coordinates.len());
        }
        
        let (xtile, ytile, xpixel, ypixel) = latlon_to_tile_coords(lat, lon, config.zoom_level);
        let tile_key = TileKey { z: config.zoom_level, x: xtile, y: ytile };
        
        match cache.get_tile(tile_key) {
            Ok(img) => {
                let terrain_elev = extract_terrarium_elevation(&img, xpixel, ypixel);
                terrain_elevations.push(terrain_elev);
            },
            Err(e) => {
                if i < 10 { // Only log first few errors to avoid spam
                    eprintln!("Failed to fetch tile for coordinate {}: {}", i, e);
                }
                // Use a reasonable fallback elevation (sea level)
                terrain_elevations.push(0.0);
            }
        }
    }
    
    println!("Terrain elevation extraction complete! Used {} unique tiles for {} coordinates", 
             unique_tiles_needed.len(), coordinates.len());
    terrain_elevations
}

/// Rolling distance-based smoothing
fn rolling_distance_smooth(
    distances: &[f64], 
    elevations: &[f64], 
    window_meters: f64
) -> Vec<f64> {
    if elevations.len() < 3 || window_meters <= 0.0 {
        return elevations.to_vec();
    }
    
    let mut smoothed = Vec::with_capacity(elevations.len());
    
    for i in 0..elevations.len() {
        let current_dist = distances[i];
        let half_window = window_meters / 2.0;
        
        // Find points within the distance window
        let mut window_elevations = Vec::new();
        let mut window_weights = Vec::new();
        
        for j in 0..elevations.len() {
            let point_dist = distances[j];
            let dist_diff = (point_dist - current_dist).abs();
            
            if dist_diff <= half_window {
                // Weight by distance (closer points have more influence)
                let weight = if dist_diff == 0.0 {
                    1.0
                } else {
                    1.0 / (1.0 + dist_diff / 20.0) // Slightly less aggressive weight decay
                };
                
                window_elevations.push(elevations[j]);
                window_weights.push(weight);
            }
        }
        
        // Calculate weighted average
        if !window_elevations.is_empty() {
            let weighted_sum: f64 = window_elevations.iter()
                .zip(window_weights.iter())
                .map(|(elev, weight)| elev * weight)
                .sum();
            let weight_sum: f64 = window_weights.iter().sum();
            
            smoothed.push(weighted_sum / weight_sum);
        } else {
            // Fallback to original elevation if no points in window
            smoothed.push(elevations[i]);
        }
    }
    
    smoothed
}

/// Raw 100% terrain elevation data - NO smoothing applied
pub fn terrain_smooth_high_accuracy(
    _distances: &[f64], // Not used for raw data
    coordinates: &[(f64, f64, f64)]
) -> Vec<f64> {
    let config = TerrainConfig::raw();
    let terrain_elevations = get_terrain_elevations(coordinates, &config);
    
    if terrain_elevations.is_empty() {
        // Fallback to GPS elevations if terrain failed
        return coordinates.iter().map(|(_, _, gps_elev)| *gps_elev).collect();
    }
    
    // Return raw terrain data without any smoothing
    terrain_elevations
}

/// 100% terrain elevation with 150m rolling smoothing
pub fn terrain_smooth_conservative(
    distances: &[f64],
    coordinates: &[(f64, f64, f64)]
) -> Vec<f64> {
    let config = TerrainConfig::smooth_150m();
    let terrain_elevations = get_terrain_elevations(coordinates, &config);
    
    if terrain_elevations.is_empty() {
        // Fallback to GPS elevations if terrain failed
        return coordinates.iter().map(|(_, _, gps_elev)| *gps_elev).collect();
    }
    
    rolling_distance_smooth(distances, &terrain_elevations, config.smoothing_window_meters)
}

/// 100% terrain elevation with 300m rolling smoothing
pub fn terrain_smooth_moderate(
    distances: &[f64],
    coordinates: &[(f64, f64, f64)]
) -> Vec<f64> {
    let config = TerrainConfig::smooth_300m();
    let terrain_elevations = get_terrain_elevations(coordinates, &config);
    
    if terrain_elevations.is_empty() {
        // Fallback to GPS elevations if terrain failed
        return coordinates.iter().map(|(_, _, gps_elev)| *gps_elev).collect();
    }
    
    rolling_distance_smooth(distances, &terrain_elevations, config.smoothing_window_meters)
}

pub fn calculate_terrain_elevation_gain_loss(elevations: &[f64]) -> (f64, f64) {
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
    fn test_tile_calculation_zoom15() {
        // Test known coordinates at zoom 15
        let (xtile, ytile, xpix, ypix) = latlon_to_tile_coords(52.5, 13.4, 15); // Berlin
        assert!(xtile > 0 && ytile > 0);
        assert!(xpix <= 255 && ypix <= 255);
    }
    
    #[test]
    fn test_new_smoothing_windows() {
        let conservative_config = TerrainConfig::smooth_150m();
        assert_eq!(conservative_config.smoothing_window_meters, 150.0);
        
        let moderate_config = TerrainConfig::smooth_300m();
        assert_eq!(moderate_config.smoothing_window_meters, 300.0);
        
        let raw_config = TerrainConfig::raw();
        assert_eq!(raw_config.smoothing_window_meters, 0.0);
    }
    
    #[test]
    fn test_rolling_smooth_large_windows() {
        let distances = vec![0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0];
        let elevations = vec![100.0, 110.0, 90.0, 120.0, 105.0, 95.0, 115.0];
        
        let smoothed_150 = rolling_distance_smooth(&distances, &elevations, 150.0);
        let smoothed_300 = rolling_distance_smooth(&distances, &elevations, 300.0);
        
        assert_eq!(smoothed_150.len(), elevations.len());
        assert_eq!(smoothed_300.len(), elevations.len());
        
        // 300m smoothing should generally be smoother than 150m
        // (though this isn't guaranteed for all data patterns)
    }
}
