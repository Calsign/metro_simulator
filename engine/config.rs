#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error parsing TOML: {0}")]
    TomlParsingError(#[from] toml::de::Error),
    #[error("Error serializing TOML: {0}")]
    TomlSerializingError(#[from] toml::ser::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Config {
    pub max_depth: u32,
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
}
