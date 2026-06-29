use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use sha2::{Digest, Sha256};

pub fn legacy_hash_pin(pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"pos-abarrotes-v1:");
    hasher.update(pin.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn hash_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.trim().as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("No se pudo proteger PIN: {error}"))
}

pub fn verify_pin(pin_hash: &str, pin: &str) -> bool {
    if pin_hash.starts_with("$argon2") {
        return PasswordHash::new(pin_hash)
            .ok()
            .and_then(|parsed| {
                Argon2::default()
                    .verify_password(pin.trim().as_bytes(), &parsed)
                    .ok()
            })
            .is_some();
    }
    pin_hash == legacy_hash_pin(pin)
}
