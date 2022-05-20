#[derive(Debug)]
pub enum Error {
    HttpError(reqwest::Error),
    CsrfError,
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, w: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            HttpError(e) => write!(w, "HTTP error: {}", e),
            CsrfError => write!(w, "cannot get CSRF token"),
        }
    }
}

impl std::error::Error for Error {}
