/// Terrain-based elevation correction using Terrarium/Terrain-RGB DEM tiles
/// Provides ground truth elevation data from satellite/aerial DEM sources
use reqwest::blocking::get;
use image::io::Reader as ImgReader;
use image::{DynamicImage, Rgba};
use std::io::Cursor;
use anyhow::{Result, anyhow};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TerrainElevationConfig {
    /// Zoom level for DEM tiles (higher = more accurate but more requests)
    pub zoom_level: u8,
    /// Enable tile caching to reduce HTTP requests
    pub enable_caching: bool,
    /// Blend factor between GPS and terrain elevation (0.0 = all terrain, 1.0 = all GPS)
    pub gps_terrain_blend_factor: f64,
    /// Maximum elevation difference to trust terrain over GPS (meters)
    pub max_terrain_gps_diff_m: f64,
    /// Fallback to GPS if terrain fetch fails
    pub fallback_to_gps: bool,
}

impl Default for TerrainElevationConfig {
    fn default() -> Self {
        TerrainElevationConfig {
            zoom_level: 12,  // Good balance of accuracy vs requests
            enable_caching: true,
            gps_terrain_blend_factor: 0.3,  // 30% GPS, 70% terrain
            max_terrain_gps_diff_m: 100.0,  // Trust terrain if diff < 100m
            fallback_to_gps: true,
        }
    }
}

impl TerrainElevationConfig {
    /// High accuracy - more requests but better precision
    pub fn high_accuracy() -> Self {
        TerrainElevationConfig {
            zoom_level: 14,
            gps_terrain_blend_factor: 0.2,  // Trust terrain more
            max_terrain_gps_diff_m: 200.0,
            ..Default::default()
        }
    }
    
    /// Conservative - fewer requests, blend more with GPS
    pub fn conservative() -> Self {
        TerrainElevationConfig {
            zoom_level: 10,
            gps_terrain_blend_factor: 0.5,  // Equal blend
            max_terrain_gps_diff_m: 50.0,   // Stricter diff threshold
            ..Default::default()
        }
    }
}

/// Tile cache to avoid repeated HTTP requests
struct TileCache {
    cache: HashMap<String, DynamicImage>,
    enabled: bool,
}

impl TileCache {
    fn new(enabled: bool) -> Self {
        TileCache {
            cache: HashMap::new(),
            enabled,
        }
    }
    
    fn get_or_fetch(&mut self, url: &str) -> Result<DynamicImage> {
        if self.enabled {
            if let Some(img) = self.cache.get(url) {
                return Ok(img.clone());
            }
        }
        
        // Fetch tile
        let resp = get(url)?.bytes()?;
        let img = ImgReader::new(Cursor::new(resp))
            .with_guessed_format()?
            .decode()?;
        
        if self.enabled {
            self.cache.insert(url.to_string(), img.clone());
        }
        
        Ok(img)
    }
}

/// Convert lat/lon to tile coordinates and pixel offsets
fn latlon_to_tile_pixel(lat: f64, lon: f64, zoom: u8) -> (u32, u32, u32, u32) {
    let n = 2_f64.powi(zoom as i32);
    let lat_rad = lat.to_radians();
    
    let xtile = ((lon + 180.0) / 360.0 * n) as u32;
    let ytile = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n) as u32;
    
    // Calculate pixel position within tile (tiles are typically 256x256)
    let tile_size = 256.0;
    let x_frac = (lon + 180.0) / 360.0 * n - xtile as f64;
    let y_frac = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n - ytile as f64;
    
    let xpixel = (x_frac * tile_size) as u32;
    let ypixel = (y_frac * tile_size) as u32;
    
    (xtile, ytile, xpixel.min(255), ypixel.min(255))
}

/// Fetch elevation from Terrarium DEM tiles
fn fetch_terrarium_elevation(
    lat: f64, 
    lon: f64, 
    zoom: u8, 
    cache: &mut TileCache
) -> Result<f64> {
    // Compute tile coordinates
    let (xtile, ytile, xpixel, ypixel) = latlon_to_tile_pixel(lat, lon, zoom);
    
    // Terrarium tile URL
    let url = format!(
        "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{}/{}/{}.png",
        zoom, xtile, ytile
    );
    
    // Fetch and decode tile
    let img = cache.get_or_fetch(&url)?;
    
    // Get pixel RGB values
    let pixel = img.get_pixel(xpixel, ypixel);
    let rgba = pixel.0;
    let (r, g, b) = (rgba[0] as u32, rgba[1] as u32, rgba[2] as u32);
    
    // Terrarium formula: elevation = (R * 256 + G + B/256) - 32768
    let elevation = (r * 256 + g) as f64 + (b as f64 / 256.0) - 32768.0;
    
    Ok(elevation)
}

/// Fetch elevation from Mapbox Terrain-RGB tiles (alternative)
fn fetch_mapbox_terrain_elevation(
    lat: f64, 
    lon: f64, 
    zoom: u8, 
    cache: &mut TileCache
) -> Result<f64> {
    let (xtile, ytile, xpixel, ypixel) = latlon_to_tile_pixel(lat, lon, zoom);
    
    // Note: Mapbox requires API key for production use
    // This is just the formula - you'd need to replace with actual Mapbox URL + API key
    let url = format!(
        "https://api.mapbox.com/v4/mapbox.terrain-rgb/{}/{}/{}.png?access_token=YOUR_TOKEN",
        zoom, xtile, ytile
    );
    
    let img = cache.get_or_fetch(&url)?;
    let pixel = img.get_pixel(xpixel, ypixel);
    let rgba = pixel.0;
    let (r, g, b) = (rgba[0] as u32, rgba[1] as u32, rgba[2] as u32);
    
    // Mapbox Terrain-RGB formula: elevation = -10000 + (R * 256² + G * 256 + B) * 0.1
    let elevation = -10000.0 + (r * 256 * 256 + g * 256 + b) as f64 * 0.1;
    
    Ok(elevation)
}

/// Apply terrain-based elevation correction to GPS coordinates
pub fn apply_terrain_correction(
    coordinates: &[(f64, f64, f64)], // (lat, lon, gps_elevation)
    config: &TerrainElevationConfig
) -> Vec<(f64, f64, f64)> {
    let mut cache = TileCache::new(config.enable_caching);
    let mut corrected = Vec::with_capacity(coordinates.len());
    
    println!("Applying terrain-based elevation correction...");
    
    for (i, &(lat, lon, gps_elev)) in coordinates.iter().enumerate() {
        if i % 100 == 0 {
            println!("Processing point {} of {}", i, coordinates.len());
        }
        
        match fetch_terrarium_elevation(lat, lon, config.zoom_level, &mut cache) {
            Ok(terrain_elev) => {
                // Check if terrain elevation is reasonable compared to GPS
                let elevation_diff = (terrain_elev - gps_elev).abs();
                
                if elevation_diff <= config.max_terrain_gps_diff_m {
                    // Blend GPS and terrain elevations
                    let corrected_elev = config.gps_terrain_blend_factor * gps_elev + 
                                       (1.0 - config.gps_terrain_blend_factor) * terrain_elev;
                    corrected.push((lat, lon, corrected_elev));
                } else if config.fallback_to_gps {
                    // Large difference - trust GPS (might be indoor/urban canyon)
                    corrected.push((lat, lon, gps_elev));
                } else {
                    // Use terrain elevation anyway
                    corrected.push((lat, lon, terrain_elev));
                }
            },
            Err(_) => {
                // Terrain fetch failed - use GPS elevation
                if config.fallback_to_gps {
                    corrected.push((lat, lon, gps_elev));
                } else {
                    // Skip this point or interpolate
                    corrected.push((lat, lon, gps_elev));
                }
            }
        }
    }
    
    println!("Terrain correction complete!");
    corrected
}

/// Simple terrain-based smoothing using corrected elevations
pub fn terrain_based_smooth(
    distances: &[f64],
    coordinates: &[(f64, f64, f64)], // (lat, lon, gps_elevation)
    config: &TerrainElevationConfig
) -> Vec<f64> {
    // Apply terrain correction first
    let corrected_coords = apply_terrain_correction(coordinates, config);
    
    // Extract corrected elevations
    let corrected_elevations: Vec<f64> = corrected_coords.iter()
        .map(|(_, _, elev)| *elev)
        .collect();
    
    // Apply light smoothing to the terrain-corrected data
    light_smooth(&corrected_elevations, 3)
}

/// Light smoothing for terrain-corrected data
fn light_smooth(elevations: &[f64], window: usize) -> Vec<f64> {
    if elevations.len() < window {
        return elevations.to_vec();
    }
    
    let mut result = Vec::with_capacity(elevations.len());
    
    for i in 0..elevations.len() {
        let start = if i >= window / 2 { i - window / 2 } else { 0 };
        let end = if i + window / 2 < elevations.len() { i + window / 2 } else { elevations.len() - 1 };
        
        let sum: f64 = elevations[start..=end].iter().sum();
        let count = end - start + 1;
        result.push(sum / count as f64);
    }
    
    result
}

/// Convenience functions with different accuracy levels
pub fn terrain_smooth_high_accuracy(
    distances: &[f64],
    coordinates: &[(f64, f64, f64)]
) -> Vec<f64> {
    terrain_based_smooth(distances, coordinates, &TerrainElevationConfig::high_accuracy())
}

pub fn terrain_smooth_moderate(
    distances: &[f64],
    coordinates: &[(f64, f64, f64)]
) -> Vec<f64> {
    terrain_based_smooth(distances, coordinates, &TerrainElevationConfig::default())
}

pub fn terrain_smooth_conservative(
    distances: &[f64],
    coordinates: &[(f64, f64, f64)]
) -> Vec<f64> {
    terrain_based_smooth(distances, coordinates, &TerrainElevationConfig::conservative())
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
    fn test_tile_calculation() {
        // Test known coordinates
        let (xtile, ytile, xpix, ypix) = latlon_to_tile_pixel(52.5, 13.4, 12); // Berlin
        assert!(xtile > 0 && ytile > 0);
        assert!(xpix <= 255 && ypix <= 255);
    }
    
    #[test]
    fn test_elevation_calculation() {
        // Test Terrarium formula with known values
        let r = 100u32;
        let g = 50u32; 
        let b = 25u32;
        let elevation = (r * 256 + g) as f64 + (b as f64 / 256.0) - 32768.0;
        
        // Should be reasonable elevation value
        assert!(elevation > -1000.0 && elevation < 10000.0);
    }
}

/// Error handling for terrain elevation operations
#[derive(Debug)]
pub enum TerrainError {
    NetworkError(String),
    InvalidCoordinates,
    TileNotFound,
    DecodingError(String),
}

impl std::fmt::Display for TerrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TerrainError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            TerrainError::InvalidCoordinates => write!(f, "Invalid coordinates"),
            TerrainError::TileNotFound => write!(f, "DEM tile not found"),
            TerrainError::DecodingError(msg) => write!(f, "Image decoding error: {}", msg),
        }
    }
}

impl std::error::Error for TerrainError {}
