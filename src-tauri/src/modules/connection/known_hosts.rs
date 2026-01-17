use std::path::Path;
use std::fs::{self, File};
use std::io::{self, Write};
use anyhow::anyhow;
use russh_keys::key::PublicKey;
use std::collections::HashMap;
use dirs::home_dir;
use log::{info, debug};

/// KnownHostsManager handles the management of known SSH hosts and their keys
#[derive(Clone)]
pub struct KnownHostsManager {
    known_hosts_path: String,
    known_hosts: HashMap<String, Vec<PublicKey>>,
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
        let _reader = io::BufReader::new(file);
        
        info!("Loading known_hosts from: {}", self.known_hosts_path);
        
        // For now, we'll use a simple implementation
        // In a more complete implementation, we would parse the known_hosts file
        // and store the host keys in the known_hosts HashMap
        
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
                // Format: host key-type base64-key comment
                let key_type = match key {
                    PublicKey::Ed25519(_) => "ssh-ed25519",
                    _ => "unknown",
                };
                
                // For now, we'll just write a simple format
                // In a more complete implementation, we would use the proper SSH key format
                writeln!(file, "{} {}", host, key_type)?;
            }
        }
        
        Ok(())
    }
    
    /// Check if a host key is known and valid
    pub fn check_host_key(&self, host: &str, port: u16, key: &PublicKey) -> Result<bool, anyhow::Error> {
        let host_patterns = [
            format!("[{}]:{}", host, port),
            host.to_string(),
        ];
        
        for host_pattern in host_patterns {
            debug!("Checking host key for: {}", host_pattern);
            
            if let Some(keys) = self.known_hosts.get(&host_pattern) {
                if keys.contains(key) {
                    info!("Host key is known and valid for: {}", host_pattern);
                    return Ok(true);
                }
            }
        }
        
        // Key not found
        info!("Host key not found in known_hosts for: {}", host);
        Ok(false)
    }
    
    /// Add a host key to known_hosts
    pub fn add_host_key(&mut self, host: &str, port: u16, key: &PublicKey) -> Result<(), anyhow::Error> {
        let host_pattern = format!("[{}]:{}", host, port);
        info!("Adding host key for: {}", host_pattern);
        
        // Add the key to the in-memory map
        self.known_hosts
            .entry(host_pattern.clone())
            .or_insert_with(Vec::new)
            .push(key.clone());
        
        // Save to file
        self.save()?;
        
        info!("Successfully added host key to known_hosts");
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