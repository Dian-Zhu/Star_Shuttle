use anyhow::anyhow;
use base64::Engine as _;
use futures::Future;
use russh::client::{Handle, KeyboardInteractiveAuthResponse};
#[cfg(unix)]
use russh_keys::agent::client::AgentClient;
use russh_keys::encoding::Encoding;
use russh_keys::key::PublicKey;
use russh_keys::key::SignatureHash;
use russh_keys::load_secret_key;
use russh_keys::signature::Signature;
use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
#[cfg(unix)]
use tokio::net::UnixStream;

use super::{KeyboardInteractivePromptRequest, KeyboardInteractivePrompter, SshHandler};

#[derive(Clone)]
pub(super) struct KeyPairSigner {
    pub(super) key_pair: Arc<russh_keys::key::KeyPair>,
}

fn append_signature(
    data: &mut russh::CryptoVec,
    signature: &Signature,
) -> Result<(), anyhow::Error> {
    let (t, sig) = match signature {
        Signature::Ed25519(bytes) => (&b"ssh-ed25519"[..], bytes.0.as_slice()),
        Signature::P256(bytes) => (&b"ecdsa-sha2-nistp256"[..], bytes.as_slice()),
        Signature::RSA { hash, bytes } => {
            let t = match hash {
                SignatureHash::SHA2_256 => &b"rsa-sha2-256"[..],
                SignatureHash::SHA2_512 => &b"rsa-sha2-512"[..],
                SignatureHash::SHA1 => &b"ssh-rsa"[..],
            };
            (t, bytes.as_slice())
        }
    };

    data.push_u32_be((t.len() + sig.len() + 8) as u32);
    data.extend_ssh_string(t);
    data.extend_ssh_string(sig);
    Ok(())
}

impl russh::Signer for KeyPairSigner {
    type Error = anyhow::Error;
    type Future =
        Pin<Box<dyn Future<Output = (Self, Result<russh::CryptoVec, Self::Error>)> + Send>>;

    fn auth_publickey_sign(self, _key: &PublicKey, mut to_sign: russh::CryptoVec) -> Self::Future {
        let key_pair = self.key_pair.clone();
        Box::pin(async move {
            let signature = match key_pair.sign_detached(&to_sign) {
                Ok(s) => s,
                Err(e) => {
                    return (self, Err(anyhow!("Failed to sign with private key: {}", e)));
                }
            };
            if let Err(e) = append_signature(&mut to_sign, &signature) {
                return (self, Err(e));
            }
            (self, Ok(to_sign))
        })
    }
}

pub(super) fn load_openssh_public_key(path: &str) -> Result<PublicKey, anyhow::Error> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow!("Failed to read public key file {}: {}", path, e))?;
    let line = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .ok_or_else(|| anyhow!("Public key file has no key line: {}", path))?;

    let mut parts = line.split_whitespace();
    let key_type = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid public key line (missing type): {}", path))?;
    if key_type.contains("-cert-v01@openssh.com") {
        return Err(anyhow!(
            "OpenSSH certificate public keys are not supported: {}",
            key_type
        ));
    }
    let key_base64 = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid public key line (missing base64): {}", path))?;

    let raw = base64::engine::general_purpose::STANDARD
        .decode(key_base64)
        .map_err(|e| anyhow!("Invalid base64 in public key file {}: {}", path, e))?;

    if raw.len() < 4 {
        return Err(anyhow!("Invalid SSH public key blob (too short): {}", path));
    }
    let algo_len = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    if raw.len() < 4 + algo_len {
        return Err(anyhow!(
            "Invalid SSH public key blob (bad algo length): {}",
            path
        ));
    }
    let algo = &raw[4..4 + algo_len];
    let pubkey = &raw[4 + algo_len..];

    let pk = PublicKey::parse(algo, pubkey)
        .map_err(|e| anyhow!("Failed to parse public key {}: {}", path, e))?;
    if pk.name() != key_type {
        log::debug!(
            "Public key type mismatch: file says {}, parsed says {}",
            key_type,
            pk.name()
        );
    }

    Ok(pk)
}

pub(super) fn load_private_key(
    key_path: &str,
    passphrase: Option<&str>,
) -> Result<russh_keys::key::KeyPair, anyhow::Error> {
    load_secret_key(key_path, passphrase)
        .map_err(|e| anyhow!("Failed to load private key: {:?}", e))
}

pub(super) fn validate_certificate_paths(
    cert_path: &str,
    key_path: &str,
) -> Result<(), anyhow::Error> {
    let cert_path_obj = Path::new(cert_path);
    if !cert_path_obj.exists() {
        return Err(anyhow!(
            "Certificate file not found: {}",
            cert_path_obj.display()
        ));
    }
    let cert_meta = fs::metadata(cert_path_obj)
        .map_err(|e| anyhow!("Failed to read certificate metadata: {}", e))?;
    if cert_meta.len() == 0 {
        return Err(anyhow!(
            "Certificate file is empty: {}",
            cert_path_obj.display()
        ));
    }

    let key_path_obj = Path::new(key_path);
    if !key_path_obj.exists() {
        return Err(anyhow!(
            "Private key file not found: {}",
            key_path_obj.display()
        ));
    }
    let key_meta = fs::metadata(key_path_obj)
        .map_err(|e| anyhow!("Failed to read private key metadata: {}", e))?;
    if key_meta.len() == 0 {
        return Err(anyhow!(
            "Private key file is empty: {}",
            key_path_obj.display()
        ));
    }

    Ok(())
}

pub(super) async fn authenticate_keyboard_interactive(
    handle: &mut Handle<SshHandler>,
    host: &str,
    port: u16,
    username: &str,
    prompter: Arc<dyn KeyboardInteractivePrompter>,
) -> Result<bool, anyhow::Error> {
    let mut response = handle
        .authenticate_keyboard_interactive_start(username, None::<String>)
        .await
        .map_err(anyhow::Error::from)?;

    loop {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let replies = prompter
                    .prompt(KeyboardInteractivePromptRequest {
                        host: host.to_string(),
                        port,
                        username: username.to_string(),
                        name,
                        instructions,
                        prompts,
                    })
                    .await?;
                response = handle
                    .authenticate_keyboard_interactive_respond(replies)
                    .await
                    .map_err(anyhow::Error::from)?;
            }
        }
    }
}

#[cfg(unix)]
pub(super) async fn authenticate_agent(
    handle: &mut Handle<SshHandler>,
    username: &str,
    agent_path: Option<String>,
) -> Result<bool, anyhow::Error> {
    let sock = match agent_path {
        Some(p) => p,
        None => std::env::var("SSH_AUTH_SOCK").map_err(|_| anyhow!("SSH_AUTH_SOCK not set"))?,
    };
    let stream = UnixStream::connect(sock).await?;
    let mut client = AgentClient::connect(stream);
    let keys = client.request_identities().await?;
    let public_key = keys
        .into_iter()
        .next()
        .ok_or(anyhow!("No identities in SSH agent"))?;
    let (_, res) = handle
        .authenticate_future(username, public_key, client)
        .await;
    res.map_err(anyhow::Error::from)
}
