use anyhow::anyhow;
use dirs::home_dir;
use log::{debug, info};
use russh_keys::key::PublicKey;
use russh_keys::PublicKeyBase64;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
struct KnownHostKey {
    key_type: String,
    key_base64: String,
}

/// KnownHostsManager handles the management of known SSH hosts and their keys
#[derive(Clone)]
pub struct KnownHostsManager {
    known_hosts_path: String,
    known_hosts: HashMap<String, Vec<KnownHostKey>>,
}

impl KnownHostsManager {
    /// Create a new KnownHostsManager instance
    pub fn new() -> Result<Self, anyhow::Error> {
        // Get the default known_hosts path
        let known_hosts_path = home_dir()
            .ok_or_else(|| anyhow!("Failed to get home directory"))?
            .join(".ssh")
            .join("known_hosts")
            .to_string_lossy()
            .to_string();

        info!("Using known_hosts file at: {}", known_hosts_path);

        let mut manager = Self {
            known_hosts_path,
            known_hosts: HashMap::new(),
        };

        // Load known hosts from file if it exists
        manager.load()?;

        Ok(manager)
    }

    /// Load known hosts from file
    pub fn load(&mut self) -> Result<(), anyhow::Error> {
        if !Path::new(&self.known_hosts_path).exists() {
            info!("Known_hosts file does not exist, will create it when needed");
            return Ok(());
        }

        let file = File::open(&self.known_hosts_path)?;
        let reader = io::BufReader::new(file);

        info!("Loading known_hosts from: {}", self.known_hosts_path);

        self.known_hosts.clear();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.is_empty() {
                continue;
            }

            if parts[0].starts_with('@') {
                parts.remove(0);
            }

            if parts.len() < 3 {
                continue;
            }

            let hosts = parts[0];
            let key_type = parts[1];
            let key_base64 = parts[2];

            for host in hosts.split(',').filter(|h| !h.is_empty()) {
                self.known_hosts
                    .entry(host.to_string())
                    .or_default()
                    .push(KnownHostKey {
                        key_type: key_type.to_string(),
                        key_base64: key_base64.to_string(),
                    });
            }
        }

        Ok(())
    }

    /// Save known hosts to file
    pub fn save(&self) -> Result<(), anyhow::Error> {
        info!("Saving known_hosts to: {}", self.known_hosts_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = Path::new(&self.known_hosts_path).parent() {
            fs::create_dir_all(parent)?;
        }

        // Open file for writing
        let mut file = File::create(&self.known_hosts_path)?;

        // Write each host and its keys to the file
        for (host, keys) in &self.known_hosts {
            for key in keys {
                writeln!(file, "{} {} {}", host, key.key_type, key.key_base64)?;
            }
        }

        Ok(())
    }

    /// Check if a host key is known and valid
    pub fn check_host_key(
        &self,
        host: &str,
        port: u16,
        key: &PublicKey,
    ) -> Result<bool, anyhow::Error> {
        let host_patterns = [format!("[{}]:{}", host, port), host.to_string()];

        let (key_type, key_base64) = public_key_parts(key)?;
        let candidate = KnownHostKey {
            key_type,
            key_base64,
        };

        for host_pattern in host_patterns {
            debug!("Checking host key for: {}", host_pattern);

            if let Some(keys) = self.known_hosts.get(&host_pattern) {
                if keys.contains(&candidate) {
                    info!("Host key is known and valid for: {}", host_pattern);
                    return Ok(true);
                }
                return Err(anyhow!("Host key mismatch for: {}", host_pattern));
            }
        }

        // Key not found
        info!("Host key not found in known_hosts for: {}", host);
        Ok(false)
    }

    /// Add a host key to known_hosts
    pub fn add_host_key(
        &mut self,
        host: &str,
        port: u16,
        key: &PublicKey,
    ) -> Result<(), anyhow::Error> {
        let host_pattern = format!("[{}]:{}", host, port);
        info!("Adding host key for: {}", host_pattern);

        let (key_type, key_base64) = public_key_parts(key)?;

        // Add the key to the in-memory map
        self.known_hosts
            .entry(host_pattern.clone())
            .or_default()
            .push(KnownHostKey {
                key_type,
                key_base64,
            });

        // Save to file
        self.save()?;

        info!("Successfully added host key to known_hosts");
        Ok(())
    }

    pub fn upsert_host_key_parts(
        &mut self,
        host: &str,
        port: u16,
        key_type: String,
        key_base64: String,
        replace: bool,
    ) -> Result<(), anyhow::Error> {
        let host_pattern = format!("[{}]:{}", host, port);

        if replace {
            self.known_hosts.insert(
                host_pattern.clone(),
                vec![KnownHostKey {
                    key_type,
                    key_base64,
                }],
            );
        } else {
            let keys = self.known_hosts.entry(host_pattern.clone()).or_default();
            let candidate = KnownHostKey {
                key_type,
                key_base64,
            };
            if !keys.contains(&candidate) {
                keys.push(candidate);
            }
        }

        self.save()?;
        Ok(())
    }

    /// Remove a host key from known_hosts
    pub fn remove_host_key(&mut self, host: &str) -> Result<(), anyhow::Error> {
        info!("Removing host: {}", host);

        // Remove all host patterns for this host
        let host_patterns = [
            host.to_string(),
            format!("[{}]:*", host), // Remove all port variants
        ];

        for host_pattern in host_patterns {
            if self.known_hosts.remove(&host_pattern).is_some() {
                info!("Successfully removed host pattern: {}", host_pattern);
            }
        }

        // Save changes to file
        self.save()?;

        info!("Successfully removed host key from known_hosts");
        Ok(())
    }

    /// Get the fingerprint of a public key
    pub fn get_key_fingerprint(key: &PublicKey) -> String {
        key.fingerprint()
    }

    /// Get the algorithm of a public key
    pub fn get_key_algorithm(key: &PublicKey) -> String {
        match key {
            PublicKey::Ed25519(_) => "Ed25519".to_string(),
            _ => "Unknown".to_string(),
        }
    }
}

fn public_key_parts(key: &PublicKey) -> Result<(String, String), anyhow::Error> {
    Ok((key.name().to_string(), key.public_key_base64()))
}
