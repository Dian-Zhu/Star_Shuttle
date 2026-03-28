use crate::modules::connection::telnet::TelnetConnection;
use crate::modules::connection::{ConnectionError, ProxyType};
use log::{debug, error, info};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;

pub(crate) fn immediate_hop(proxy_type: &ProxyType, host: &str, port: u16) -> (String, u16) {
    match proxy_type {
        ProxyType::None => (host.to_string(), port),
        ProxyType::Socks5 { host, port, .. } => (host.clone(), *port),
        ProxyType::Http { host, port, .. } => (host.clone(), *port),
        ProxyType::JumpHost { host, port, .. } => (host.clone(), *port),
    }
}

pub(crate) fn preflight_connectivity_check(
    runtime: &Runtime,
    check_host: &str,
    check_port: u16,
) -> Result<(), ConnectionError> {
    info!(
        "Checking network connectivity to {}:{} before connection...",
        check_host, check_port
    );
    let addr = format!("{}:{}", check_host, check_port);
    let check_res = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(3), TcpStream::connect(&addr)).await
    });

    match check_res {
        Ok(Ok(_)) => {
            debug!(
                "Network connectivity check passed for {}:{}",
                check_host, check_port
            );
            Ok(())
        }
        Ok(Err(e)) => {
            let msg = format!(
                "网络不可达: 无法连接到 {}:{} ({})",
                check_host, check_port, e
            );
            error!("{}", msg);
            Err(ConnectionError::ConnectionFailed(msg))
        }
        Err(_) => {
            let msg = format!("网络不可达: 连接 {}:{} 超时 (3秒)", check_host, check_port);
            error!("{}", msg);
            Err(ConnectionError::ConnectionFailed(msg))
        }
    }
}

pub(crate) fn connect_telnet(
    runtime: &Runtime,
    addr: &str,
) -> Result<TelnetConnection, ConnectionError> {
    let connect_res = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(addr)).await
    });

    match connect_res {
        Ok(Ok(stream)) => {
            let (read, write) = stream.into_split();
            Ok(TelnetConnection { read, write })
        }
        Ok(Err(e)) => Err(ConnectionError::ConnectionFailed(e.to_string())),
        Err(_) => Err(ConnectionError::ConnectionFailed(
            "Telnet connection timed out".to_string(),
        )),
    }
}
