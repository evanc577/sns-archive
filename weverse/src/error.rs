#[derive(Debug)]
pub enum WeverseError {
    Login,
    Auth,
    SavedAuthFile,
}

impl std::error::Error for WeverseError {}

impl std::fmt::Display for WeverseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Login => write!(f, "failed to log in to weverse"),
            Self::Auth => write!(f, "failed to authenticate with weverse"),
            Self::SavedAuthFile => write!(f, "failed to get weverse saved auth file"),
        }
    }
}
