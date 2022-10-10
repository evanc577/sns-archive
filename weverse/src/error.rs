#[derive(Debug)]
pub enum WeverseError {
    Auth,
}

impl std::error::Error for WeverseError {}

impl std::fmt::Display for WeverseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth => write!(f, "failed to authenticate with weverse"),
        }
    }
}
