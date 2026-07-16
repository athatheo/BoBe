use crate::error::AppError;

pub(crate) fn install_crypto_provider() -> Result<(), AppError> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    match rustls::crypto::ring::default_provider().install_default() {
        Ok(()) => Ok(()),
        Err(_) if rustls::crypto::CryptoProvider::get_default().is_some() => Ok(()),
        Err(_) => Err(AppError::Internal(
            "failed to install the rustls Ring crypto provider".into(),
        )),
    }
}
