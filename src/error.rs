use thiserror::Error;

#[derive(Error, Debug)]
pub enum BitStableError {
    #[error("Insufficient collateral: required {required}, provided {provided}")]
    InsufficientCollateral { required: f64, provided: f64 },

    #[error("Vault not found: {0}")]
    VaultNotFound(bitcoin::Txid),

    #[error("Oracle consensus failed: {0}")]
    OracleConsensusFailure(String),

    #[error("Liquidation not possible: vault is healthy with ratio {ratio}")]
    LiquidationNotPossible { ratio: f64 },

    #[error("DLC creation failed: {0}")]
    DlcCreationFailed(String),

    #[error("Price feed error: {0}")]
    PriceFeedError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sled::Error),

    #[error("Bitcoin error: {0}")]
    BitcoinError(#[from] bitcoin::consensus::encode::Error),

    #[error("Amount parse error: {0}")]
    AmountParseError(#[from] bitcoin::amount::ParseAmountError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Public key parse error: {0}")]
    PublicKeyParseError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Vault already exists: {0}")]
    VaultAlreadyExists(bitcoin::Txid),

    #[error("Liquidation threshold not reached")]
    LiquidationThresholdNotReached,

    #[error("Oracle signature verification failed")]
    OracleSignatureVerificationFailed,

    #[error("Insufficient oracle consensus: got {got}, required {required}")]
    InsufficientOracleConsensus { got: usize, required: usize },
}

pub type Result<T> = std::result::Result<T, BitStableError>;