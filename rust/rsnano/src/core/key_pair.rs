use super::{PublicKey, RawKey, Signature};

pub struct KeyPair {
    keypair: ed25519_dalek_blake2b::Keypair,
}

impl Default for KeyPair {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let keypair = ed25519_dalek_blake2b::Keypair::generate(&mut rng);
        Self { keypair }
    }
}

impl KeyPair {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn zero() -> Self {
        Self::from_priv_key_bytes(&[0u8; 32]).unwrap()
    }

    pub fn from_priv_key_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let secret = ed25519_dalek_blake2b::SecretKey::from_bytes(bytes)
            .map_err(|_| anyhow!("could not load secret key"))?;
        let public = ed25519_dalek_blake2b::PublicKey::from(&secret);
        Ok(Self {
            keypair: ed25519_dalek_blake2b::Keypair { secret, public },
        })
    }

    pub fn from_priv_key_hex(s: impl AsRef<str>) -> anyhow::Result<Self> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.as_ref(), &mut bytes)?;
        Self::from_priv_key_bytes(&bytes)
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_bytes(self.keypair.public.to_bytes())
    }

    pub fn private_key(&self) -> RawKey {
        RawKey::from_bytes(self.keypair.secret.to_bytes())
    }
}

pub fn sign_message(
    private_key: &RawKey,
    public_key: &PublicKey,
    data: &[u8],
) -> anyhow::Result<Signature> {
    let secret = ed25519_dalek_blake2b::SecretKey::from_bytes(private_key.as_bytes())
        .map_err(|_| anyhow!("could not extract secret key"))?;
    let public = ed25519_dalek_blake2b::PublicKey::from_bytes(public_key.as_bytes())
        .map_err(|_| anyhow!("could not extract public key"))?;
    let expanded = ed25519_dalek_blake2b::ExpandedSecretKey::from(&secret);
    let signature = expanded.sign(data, &public);
    Ok(Signature::from_bytes(signature.to_bytes()))
}

pub fn validate_message(
    public_key: &PublicKey,
    message: &[u8],
    signature: &Signature,
) -> anyhow::Result<()> {
    let public = ed25519_dalek_blake2b::PublicKey::from_bytes(public_key.as_bytes())
        .map_err(|_| anyhow!("could not extract public key"))?;
    let sig = ed25519_dalek_blake2b::Signature::from_bytes(&signature.to_be_bytes())
        .map_err(|_| anyhow!("invalid signature bytes"))?;
    public
        .verify_strict(message, &sig)
        .map_err(|_| anyhow!("could not verify message"))?;
    Ok(())
}

pub fn validate_message_batch(
    messages: &[Vec<u8>],
    public_keys: &[PublicKey],
    signatures: &[Signature],
    valid: &mut [i32],
) {
    let len = messages.len();
    assert!(public_keys.len() == len && signatures.len() == len && valid.len() == len);
    for i in 0..len {
        valid[i] = match validate_message(&public_keys[i], &messages[i], &signatures[i]) {
            Ok(_) => 1,
            Err(_) => 0,
        }
    }
}
