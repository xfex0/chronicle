//! Label rasters: a game's map image where every colour is one province / location id.

use std::collections::HashMap;
use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RasterError {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Png(#[from] png::DecodingError),
    #[error("unsupported image layout: {0}")]
    Layout(String),
}

/// `labels[y * width + x]` is the id at that pixel; 0 = no id (unmapped colour, sea gaps…).
#[derive(Debug, Clone, PartialEq)]
pub struct LabelRaster {
    pub width: u32,
    pub height: u32,
    pub labels: Vec<u32>,
}

impl LabelRaster {
    pub fn get(&self, x: u32, y: u32) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.labels[(y as usize) * (self.width as usize) + x as usize]
    }

    /// From packed RGB(A) pixels and a colour → id table.
    pub fn from_pixels(width: u32, height: u32, pixels: &[u8], channels: usize, colors: &HashMap<[u8; 3], u32>) -> Self {
        let labels = pixels
            .chunks_exact(channels)
            .map(|p| colors.get(&[p[0], p[1], p[2]]).copied().unwrap_or(0))
            .collect();
        Self { width, height, labels }
    }

    /// Decode a PNG (RGB, RGBA or palette; 8 or 16 bit) into labels.
    pub fn from_png(path: &Path, colors: &HashMap<[u8; 3], u32>) -> Result<Self, RasterError> {
        let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info()?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf)?;
        let channels = match info.color_type {
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            other => return Err(RasterError::Layout(format!("{other:?} (expected RGB/RGBA)"))),
        };
        Ok(Self::from_pixels(info.width, info.height, &buf[..info.buffer_size()], channels, colors))
    }

    /// Replace every label through `map` (e.g. CK3 province → county); unknown → 0.
    pub fn relabel(&self, map: &HashMap<u32, u32>) -> Self {
        Self { width: self.width, height: self.height, labels: self.labels.iter().map(|l| map.get(l).copied().unwrap_or(0)).collect() }
    }
}
