use anyhow::anyhow;
use dirs::{data_local_dir, home_dir};
use fs2::FileExt;
use log::{debug, info};
use russh_keys::key::PublicKey;
use russh_keys::PublicKeyBase64;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
        let known_hosts_path = storage_path()?;

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
        info!("Loading known_hosts from: {}", self.known_hosts_path);

        let path = Path::new(&self.known_hosts_path);
        self.known_hosts = read_known_hosts_map(path)?;
        Ok(())
    }

    /// Save known hosts to file
    pub fn save(&mut self) -> Result<(), anyhow::Error> {
        info!("Saving known_hosts to: {}", self.known_hosts_path);

        let path = Path::new(&self.known_hosts_path);
        let _lock = lock_known_hosts_file(path)?;
        let mut merged = read_known_hosts_map(path)?;
        merge_known_hosts(&mut merged, &self.known_hosts);
        write_known_hosts_map_atomically(path, &merged)?;
        self.known_hosts = merged;
        Ok(())
    }

    #[cfg(test)]
    fn remove_host_key_from_latest(&mut self, host: &str) -> Result<(), anyhow::Error> {
        let path = Path::new(&self.known_hosts_path);
        let _lock = lock_known_hosts_file(path)?;
        let mut latest = read_known_hosts_map(path)?;
        latest.retain(|pattern, _| !host_pattern_matches_host(pattern, host));
        write_known_hosts_map_atomically(path, &latest)?;
        self.known_hosts = latest;
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

    pub fn upsert_host_key_parts(
        &mut self,
        host: &str,
        port: u16,
        key_type: String,
        key_base64: String,
        replace: bool,
    ) -> Result<(), anyhow::Error> {
        validate_host_component("host", host)?;
        validate_token_component("key_type", &key_type)?;
        validate_token_component("key_base64", &key_base64)?;

        let host_pattern = format!("[{}]:{}", host, port);
        let candidate = KnownHostKey {
            key_type,
            key_base64,
        };

        if replace {
            self.upsert_host_key_on_latest(host_pattern, candidate, true)?;
            return Ok(());
        }

        let keys = self.known_hosts.entry(host_pattern.clone()).or_default();
        if !keys.contains(&candidate) {
            keys.push(candidate);
        }

        self.save()?;
        Ok(())
    }

    fn upsert_host_key_on_latest(
        &mut self,
        host_pattern: String,
        candidate: KnownHostKey,
        replace: bool,
    ) -> Result<(), anyhow::Error> {
        let path = Path::new(&self.known_hosts_path);
        let _lock = lock_known_hosts_file(path)?;
        let mut latest = read_known_hosts_map(path)?;

        if replace {
            latest.insert(host_pattern, vec![candidate]);
        } else {
            append_known_host_key(&mut latest, host_pattern, candidate);
        }

        write_known_hosts_map_atomically(path, &latest)?;
        self.known_hosts = latest;
        Ok(())
    }

    /// Remove a host key from known_hosts
    #[cfg(test)]
    pub fn remove_host_key(&mut self, host: &str) -> Result<(), anyhow::Error> {
        info!("Removing host: {}", host);

        let host_patterns: Vec<String> = self
            .known_hosts
            .keys()
            .filter(|pattern| host_pattern_matches_host(pattern, host))
            .cloned()
            .collect();

        for host_pattern in host_patterns {
            self.known_hosts.remove(&host_pattern);
            info!("Successfully removed host pattern: {}", host_pattern);
        }

        // Save changes to file using lock + latest-state rewrite semantics.
        self.remove_host_key_from_latest(host)?;

        info!("Successfully removed host key from known_hosts");
        Ok(())
    }
}

fn read_known_hosts_map(path: &Path) -> Result<HashMap<String, Vec<KnownHostKey>>, anyhow::Error> {
    if !path.exists() {
        info!("Known_hosts file does not exist, will create it when needed");
        return Ok(HashMap::new());
    }

    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    parse_known_hosts_reader(reader)
}

fn parse_known_hosts_reader<R: BufRead>(
    reader: R,
) -> Result<HashMap<String, Vec<KnownHostKey>>, anyhow::Error> {
    let mut known_hosts = HashMap::new();

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
            append_known_host_key(
                &mut known_hosts,
                host.to_string(),
                KnownHostKey {
                    key_type: key_type.to_string(),
                    key_base64: key_base64.to_string(),
                },
            );
        }
    }

    Ok(known_hosts)
}

fn append_known_host_key(
    known_hosts: &mut HashMap<String, Vec<KnownHostKey>>,
    host: String,
    key: KnownHostKey,
) {
    let keys = known_hosts.entry(host).or_default();
    if !keys.contains(&key) {
        keys.push(key);
    }
}

fn merge_known_hosts(
    base: &mut HashMap<String, Vec<KnownHostKey>>,
    incoming: &HashMap<String, Vec<KnownHostKey>>,
) {
    for (host, keys) in incoming {
        for key in keys {
            append_known_host_key(base, host.clone(), key.clone());
        }
    }
}

fn known_hosts_lock_path(path: &Path) -> PathBuf {
    let mut lock = path.as_os_str().to_os_string();
    lock.push(".lock");
    PathBuf::from(lock)
}

fn lock_known_hosts_file(path: &Path) -> Result<File, anyhow::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        ensure_private_dir_permissions(parent)?;
    }

    let lock_path = known_hosts_lock_path(path);
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock_file = options.open(&lock_path)?;
    ensure_private_file_permissions(&lock_path)?;
    lock_file.lock_exclusive()?;
    Ok(lock_file)
}

fn write_known_hosts_map_atomically(
    path: &Path,
    map: &HashMap<String, Vec<KnownHostKey>>,
) -> Result<(), anyhow::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        ensure_private_dir_permissions(parent)?;
    }

    let temp_path = temporary_known_hosts_path(path);
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp_path)?;
    ensure_private_file_permissions(&temp_path)?;

    for (host, keys) in map {
        for key in keys {
            writeln!(file, "{} {} {}", host, key.key_type, key.key_base64)?;
        }
    }

    file.flush()?;
    file.sync_all()?;
    drop(file);

    fs::rename(&temp_path, path)?;
    ensure_private_file_permissions(path)?;
    Ok(())
}

fn temporary_known_hosts_path(path: &Path) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("known_hosts");
    let temp_name = format!("{}.tmp-{}-{}", file_name, std::process::id(), unique);
    path.with_file_name(temp_name)
}

fn storage_path() -> Result<String, anyhow::Error> {
    #[cfg(test)]
    if let Ok(path) = std::env::var("STAR_SHUTTLE_KNOWN_HOSTS_PATH") {
        return Ok(path);
    }

    let base_dir = data_local_dir().or_else(|| {
        home_dir().map(|mut path| {
            path.push(".local");
            path.push("share");
            path
        })
    });

    let path = base_dir
        .ok_or_else(|| anyhow!("Failed to determine application data directory"))?
        .join("star_shuttle")
        .join("ssh")
        .join("known_hosts");

    Ok(path.to_string_lossy().to_string())
}

fn ensure_private_dir_permissions(path: &Path) -> Result<(), anyhow::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn ensure_private_file_permissions(path: &Path) -> Result<(), anyhow::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn validate_host_component(name: &str, value: &str) -> Result<(), anyhow::Error> {
    if value.is_empty() {
        return Err(anyhow!("{name} cannot be empty"));
    }

    if value.chars().any(char::is_whitespace) || value.contains('[') || value.contains(']') {
        return Err(anyhow!("{name} contains invalid characters"));
    }

    Ok(())
}

fn validate_token_component(name: &str, value: &str) -> Result<(), anyhow::Error> {
    if value.is_empty() {
        return Err(anyhow!("{name} cannot be empty"));
    }

    if value.chars().any(char::is_whitespace) {
        return Err(anyhow!("{name} contains invalid whitespace"));
    }

    Ok(())
}

#[cfg(test)]
fn host_pattern_matches_host(pattern: &str, host: &str) -> bool {
    pattern == host
        || pattern
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .is_some_and(|(pattern_host, remainder)| {
                pattern_host == host && remainder.starts_with(':')
            })
}

fn public_key_parts(key: &PublicKey) -> Result<(String, String), anyhow::Error> {
    Ok((key.name().to_string(), key.public_key_base64()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Barrier};
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_known_hosts_path() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("star-shuttle-known-hosts-test-{}", suffix))
            .join("known_hosts")
    }

    fn manager_for_path(path: &Path) -> KnownHostsManager {
        KnownHostsManager {
            known_hosts_path: path.to_string_lossy().to_string(),
            known_hosts: HashMap::new(),
        }
    }

    #[test]
    fn uses_app_private_store_path_override() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let path = temp_known_hosts_path();
        std::env::set_var(
            "STAR_SHUTTLE_KNOWN_HOSTS_PATH",
            path.to_string_lossy().to_string(),
        );
        let manager = KnownHostsManager::new().expect("manager should initialize");
        assert_eq!(manager.known_hosts_path, path.to_string_lossy());
        std::env::remove_var("STAR_SHUTTLE_KNOWN_HOSTS_PATH");
    }

    #[test]
    fn saves_host_keys_to_private_store() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let path = temp_known_hosts_path();
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        std::env::set_var(
            "STAR_SHUTTLE_KNOWN_HOSTS_PATH",
            path.to_string_lossy().to_string(),
        );

        let mut manager = KnownHostsManager::new().expect("manager should initialize");
        manager
            .upsert_host_key_parts(
                "example.com",
                22,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIBASE64".to_string(),
                false,
            )
            .expect("save should succeed");

        let saved = fs::read_to_string(&path).expect("saved known_hosts should exist");
        assert!(saved.contains("[example.com]:22 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBASE64"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = fs::metadata(&path)
                .expect("metadata should exist")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);

            let dir_mode = fs::metadata(path.parent().expect("path should have parent"))
                .expect("parent metadata should exist")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
        }

        std::env::remove_var("STAR_SHUTTLE_KNOWN_HOSTS_PATH");
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn rejects_injected_tokens() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let path = temp_known_hosts_path();
        std::env::set_var(
            "STAR_SHUTTLE_KNOWN_HOSTS_PATH",
            path.to_string_lossy().to_string(),
        );

        let mut manager = KnownHostsManager::new().expect("manager should initialize");
        let error = manager
            .upsert_host_key_parts(
                "example.com",
                22,
                "ssh-ed25519\nssh-rsa".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIBASE64".to_string(),
                false,
            )
            .expect_err("invalid token should be rejected");
        assert!(error.to_string().contains("invalid whitespace"));

        std::env::remove_var("STAR_SHUTTLE_KNOWN_HOSTS_PATH");
    }

    #[test]
    fn remove_host_key_clears_all_port_variants() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let path = temp_known_hosts_path();
        std::env::set_var(
            "STAR_SHUTTLE_KNOWN_HOSTS_PATH",
            path.to_string_lossy().to_string(),
        );

        let mut manager = KnownHostsManager::new().expect("manager should initialize");
        manager
            .upsert_host_key_parts(
                "example.com",
                22,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIBASE64".to_string(),
                false,
            )
            .expect("first save should succeed");
        manager
            .upsert_host_key_parts(
                "example.com",
                2222,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIBASE65".to_string(),
                false,
            )
            .expect("second save should succeed");
        manager
            .known_hosts
            .insert("example.com".to_string(), Vec::new());

        manager
            .remove_host_key("example.com")
            .expect("remove should succeed");

        assert!(manager.known_hosts.is_empty());
        let saved = fs::read_to_string(&path).expect("saved known_hosts should exist");
        assert!(!saved.contains("example.com"));

        std::env::remove_var("STAR_SHUTTLE_KNOWN_HOSTS_PATH");
    }

    #[test]
    fn stale_managers_merge_without_losing_entries() {
        let path = temp_known_hosts_path();
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        let mut manager_a = manager_for_path(&path);
        let mut manager_b = manager_for_path(&path);
        manager_a.load().expect("manager A should load");
        manager_b.load().expect("manager B should load");

        manager_a
            .upsert_host_key_parts(
                "a.example.com",
                22,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIBASE64A".to_string(),
                false,
            )
            .expect("manager A save should succeed");

        manager_b
            .upsert_host_key_parts(
                "b.example.com",
                22,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIBASE64B".to_string(),
                false,
            )
            .expect("manager B save should succeed");

        let saved = fs::read_to_string(&path).expect("saved known_hosts should exist");
        assert!(saved.contains("[a.example.com]:22 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBASE64A"));
        assert!(saved.contains("[b.example.com]:22 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBASE64B"));

        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn concurrent_writers_do_not_drop_known_hosts_entries() {
        let path = temp_known_hosts_path();
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        let barrier = Arc::new(Barrier::new(2));
        let path_a = path.clone();
        let barrier_a = barrier.clone();
        let handle_a = thread::spawn(move || {
            let mut manager = manager_for_path(&path_a);
            manager.load().expect("manager A should load");
            barrier_a.wait();
            manager
                .upsert_host_key_parts(
                    "race-a.example.com",
                    22,
                    "ssh-ed25519".to_string(),
                    "AAAAC3NzaC1lZDI1NTE5AAAAIRACEA".to_string(),
                    false,
                )
                .expect("manager A save should succeed");
        });

        let path_b = path.clone();
        let barrier_b = barrier.clone();
        let handle_b = thread::spawn(move || {
            let mut manager = manager_for_path(&path_b);
            manager.load().expect("manager B should load");
            barrier_b.wait();
            manager
                .upsert_host_key_parts(
                    "race-b.example.com",
                    22,
                    "ssh-ed25519".to_string(),
                    "AAAAC3NzaC1lZDI1NTE5AAAAIRACEB".to_string(),
                    false,
                )
                .expect("manager B save should succeed");
        });

        handle_a.join().expect("manager A thread should complete");
        handle_b.join().expect("manager B thread should complete");

        let saved = fs::read_to_string(&path).expect("saved known_hosts should exist");
        assert!(
            saved.contains("[race-a.example.com]:22 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIRACEA")
        );
        assert!(
            saved.contains("[race-b.example.com]:22 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIRACEB")
        );

        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn replace_updates_key_without_reintroducing_stale_value() {
        let path = temp_known_hosts_path();
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        let mut manager = manager_for_path(&path);
        manager
            .upsert_host_key_parts(
                "replace.example.com",
                22,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAIOLDVALUE".to_string(),
                false,
            )
            .expect("initial save should succeed");

        let mut stale_manager = manager_for_path(&path);
        stale_manager.load().expect("stale manager should load");
        stale_manager
            .upsert_host_key_parts(
                "replace.example.com",
                22,
                "ssh-ed25519".to_string(),
                "AAAAC3NzaC1lZDI1NTE5AAAAINEWVALUE".to_string(),
                true,
            )
            .expect("replace save should succeed");

        let saved = fs::read_to_string(&path).expect("saved known_hosts should exist");
        assert!(saved
            .contains("[replace.example.com]:22 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINEWVALUE"));
        assert!(!saved.contains("AAAAC3NzaC1lZDI1NTE5AAAAIOLDVALUE"));

        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
