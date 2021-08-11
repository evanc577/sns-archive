use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bearer: String,
    pub directory: String,
    pub users: Vec<String>,
    #[serde(default)]
    pub timezone_offset: i32,
}

impl Config {
    pub fn read() -> Config {
        let path = "config.toml";
        let data = std::fs::read_to_string(path).expect(&format!("Unable to read {}", path));
        toml::from_str(&data).expect(&format!("Unable to parse {}", path))
    }
}
