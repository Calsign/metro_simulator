#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error parsing TOML: {0}")]
    TomlParsingError(#[from] toml::de::Error),
    #[error("Error serializing TOML: {0}")]
    TomlSerializingError(#[from] toml::ser::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /** Maximum number of times a tile can be split. */
    pub max_depth: u32,
    /** The number of real people represented by a single simulated person. */
    pub people_per_sim: u32,
    /** The size (in meters) of the smallest possible tile. */
    pub min_tile_size: u32,
}

impl Config {
    pub fn load(data: &str) -> Result<Self, Error> {
        return Ok(toml::from_str(data)?);
    }

    pub fn load_file(path: &std::path::Path) -> Result<Self, Error> {
        return Ok(Self::load(&std::fs::read_to_string(path)?)?);
    }

    pub fn dump(&self) -> Result<String, Error> {
        return Ok(toml::to_string(self)?);
    }

    pub fn dump_file(&self, path: &std::path::Path) -> Result<(), Error> {
        return Ok(std::fs::write(path, self.dump()?)?);
    }

    /**
     * The width of the map, denominated by the smallest possible tile size.
     */
    pub fn tile_width(&self) -> u32 {
        2_u32.pow(self.max_depth)
    }

    /**
     * Given a target downsampled block size, returns a factor which downsamples the map evenly,
     * i.e. by a power of two.
     */
    pub fn even_downsample(&self, block_size: f32) -> u32 {
        2_u32.pow((block_size / self.min_tile_size as f32).log2().floor() as u32)
    }
}
