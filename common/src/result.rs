#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    NetcodeTransport(#[from] renet_netcode::NetcodeTransportError),
    #[error(transparent)]
    Netcode(#[from] renet_netcode::NetcodeError),
    #[error(transparent)]
    TokenGeneration(#[from] renet_netcode::TokenGenerationError),
    #[error(transparent)]
    Encode(#[from] bincode::error::EncodeError),
    #[error(transparent)]
    Decode(#[from] bincode::error::DecodeError),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error(transparent)]
    AddrParse(#[from] std::net::AddrParseError),
    #[error(transparent)]
    FromHex(#[from] hex::FromHexError),
    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
    #[error(transparent)]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error(transparent)]
    RequestAdapter(#[from] wgpu::RequestAdapterError),
    #[error(transparent)]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error(transparent)]
    Surface(#[from] wgpu::SurfaceError),
    #[error("Invalid Key Length")]
    InvalidKeyLength,
    #[error("Invalid Character Id")]
    InvalidCharacterId,
    #[error("Invalid Character Kind")]
    InvalidCharacterKind,
    #[error("{0}, Inner: {1}")]
    Context(String, Box<Error>),
}

impl Error {
    pub fn context<S: Into<String>>(self, context: S) -> Error {
        Error::Context(context.into(), Box::new(self))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait ResultExt<T> {
    fn context<S: Into<String>>(self, context: S) -> Result<T>;
}

impl<T, E: Into<Error>> ResultExt<T> for std::result::Result<T, E> {
    fn context<S: Into<String>>(self, context: S) -> Result<T> {
        self.map_err(|e| e.into().context(context))
    }
}
