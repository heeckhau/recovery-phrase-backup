// Encryption/decryption of the master secret: a 4-round Feistel network
// with PBKDF2-HMAC-SHA256 as the round function. Applied even when the
// passphrase is empty — SLIP-39 always encrypts. See the spec's
// "Encryption of the master secret" / "Decryption of the master secret".
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::SecretBytes;

fn round_function(round: u8, passphrase: &str, salt_prefix: &[u8], r: &[u8], iterations: u32) -> SecretBytes {
    // Both inputs are secret-derived: the password embeds the passphrase,
    // the salt embeds the R half of the state.
    let mut password = Zeroizing::new(vec![round]);
    password.extend_from_slice(passphrase.as_bytes());

    let mut salt = Zeroizing::new(salt_prefix.to_vec());
    salt.extend_from_slice(r);

    let mut out = Zeroizing::new(vec![0u8; r.len()]);
    pbkdf2_hmac::<Sha256>(&password, &salt, iterations, &mut out);
    out
}

/// `salt_prefix` per spec: empty when `ext`=1 (extendable), else
/// `"shamir" || id` (id as 2 big-endian bytes).
fn salt_prefix(extendable: bool, id: u16) -> Vec<u8> {
    if extendable {
        Vec::new()
    } else {
        let mut prefix = b"shamir".to_vec();
        prefix.extend_from_slice(&id.to_be_bytes());
        prefix
    }
}

fn iterations(iteration_exponent: u8) -> u32 {
    2500u32 << iteration_exponent
}

fn xor(a: &[u8], b: &[u8]) -> SecretBytes {
    Zeroizing::new(a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect())
}

/// Encrypts `master_secret` into the encrypted master secret (EMS).
pub fn encrypt(master_secret: &[u8], passphrase: &str, iteration_exponent: u8, id: u16, extendable: bool) -> SecretBytes {
    feistel(master_secret, passphrase, iteration_exponent, id, extendable, 0..4)
}

/// Decrypts an encrypted master secret (EMS) back into the master secret.
pub fn decrypt(encrypted_master_secret: &[u8], passphrase: &str, iteration_exponent: u8, id: u16, extendable: bool) -> SecretBytes {
    feistel(encrypted_master_secret, passphrase, iteration_exponent, id, extendable, (0..4).rev())
}

fn feistel(input: &[u8], passphrase: &str, iteration_exponent: u8, id: u16, extendable: bool, rounds: impl Iterator<Item = u8>) -> SecretBytes {
    let half = input.len() / 2;
    let mut l = Zeroizing::new(input[..half].to_vec());
    let mut r = Zeroizing::new(input[half..].to_vec());
    let prefix = salt_prefix(extendable, id);
    let iters = iterations(iteration_exponent);

    for round in rounds {
        let f = round_function(round, passphrase, &prefix, &r, iters);
        let new_r = xor(&l, &f);
        l = r;
        r = new_r;
    }

    let mut output = r;
    output.extend_from_slice(&l);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret_32_bytes() -> Vec<u8> {
        (0u8..32).collect()
    }

    #[test]
    fn decrypt_undoes_encrypt() {
        let secret = secret_32_bytes();
        for extendable in [true, false] {
            for id in [0u16, 1, 12345, 32767] {
                let ems = encrypt(&secret, "TREZOR", 0, id, extendable);
                assert_eq!(ems.len(), secret.len());
                assert_ne!(*ems, secret, "a real encryption round should not be a no-op");
                let recovered = decrypt(&ems, "TREZOR", 0, id, extendable);
                assert_eq!(*recovered, secret, "extendable={extendable} id={id}");
            }
        }
    }

    #[test]
    fn empty_passphrase_still_encrypts() {
        let secret = secret_32_bytes();
        let ems = encrypt(&secret, "", 0, 42, true);
        assert_ne!(*ems, secret);
        assert_eq!(*decrypt(&ems, "", 0, 42, true), secret);
    }

    #[test]
    fn wrong_passphrase_decrypts_to_something_different_not_an_error() {
        // Documents SLIP-39's plausible-deniability property: decryption
        // is unauthenticated, so there is nothing to "fail" here.
        let secret = secret_32_bytes();
        let ems = encrypt(&secret, "right", 0, 7, true);
        let wrong = decrypt(&ems, "wrong", 0, 7, true);
        assert_ne!(*wrong, secret);
    }

    #[test]
    fn extendable_flag_changes_the_result() {
        let secret = secret_32_bytes();
        let ems_extendable = encrypt(&secret, "x", 0, 7, true);
        let ems_not = encrypt(&secret, "x", 0, 7, false);
        assert_ne!(ems_extendable, ems_not);
    }
}
