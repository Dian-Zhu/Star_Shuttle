use russh::client::{Config, Handle, Handler}; use std::sync::{Arc, Mutex}; use std::net::SocketAddr; use std::str::FromStr; use tokio::net::lookup_host; use anyhow::anyhow;

pub struct SshHandler {
    username: String,
    password: Option<String>,
    auth_complete: bool,
}

impl Handler for SshHandler {
    type Error = anyhow::Error;

    fn check_server_key<'life0, 'async_trait>(
        self,
        server_public_key: &'life0 russh_keys::key::PublicKey,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, bool), Self::Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            // TODO: Implement proper server key validation
            // For now, just accept all keys (not secure for production)
            Ok((self, true))
        })
    }

    fn auth_banner<'life0, 'async_trait>(
        self,
        banner: &'life0 str,
        session: russh::client::Session,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, russh::client::Session), Self::Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            println!("SSH Banner: {}", banner);
            Ok((self, session))
        })
    }

    fn channel_open_confirmation<'async_trait>(
        self,
        id: russh::ChannelId,
        max_packet_size: u32,
        window_size: u32,
        session: russh::client::Session,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, russh::client::Session), Self::Error>> + Send + 'async_trait>>
    where
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            println!("Channel open confirmation: id={}, max_packet_size={}, window_size={}", id, max_packet_size, window_size);
            Ok((self, session))
        })
    }

    fn data<'life0, 'async_trait>(
        self,
        channel: russh::ChannelId,
        data: &'life0 [u8],
        session: russh::client::Session,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, russh::client::Session), Self::Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            println!("Received data on channel {}: {:?}", channel, String::from_utf8_lossy(data));
            Ok((self, session))
        })
    }
}

pub async fn connect_ssh(
    host: &str,
    port: u16,
    username: &str,
    password: Option<String>,
) -> Result<Arc<Mutex<Handle<SshHandler>>>, anyhow::Error> {
    // Create SSH client config
    let config = Arc::new(Config::default());

    // Create handler
    let handler = SshHandler {
        username: username.to_string(),
        password,
        auth_complete: false,
    };

    // Parse socket address or resolve hostname
    let addr = {
        // Try to parse directly as IP address
        if let Ok(addr) = SocketAddr::from_str(&format!("{}:{}", host, port)) {
            addr
        } else {
            // Resolve hostname to IP address
            let addrs = lookup_host((host, port)).await?;
            addrs.into_iter().next().ok_or_else(|| anyhow!("Failed to resolve host: {}", host))?
        }
    };

    // Connect to SSH server
    println!("Connecting to {}:{} as {} (resolved to {:?})", host, port, username, addr);
    let handle = russh::client::connect(config, addr, handler).await
        .map_err(|e| anyhow!("Failed to connect to {}:{}: {}", host, port, e))?;

    Ok(Arc::new(Mutex::new(handle)))
}
