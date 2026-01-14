use std::net::SocketAddr;
use std::str::FromStr;

fn main() {
    // 测试不同主机名的情况
    let test_cases = [
        ("", 22),           // 空主机名
        ("localhost", 22),  // localhost
        ("127.0.0.1", 22),  // 本地IP
        ("example.com", 22),// 域名
    ];

    for (host, port) in test_cases {
        let addr_str = format!("{}:{}", host, port);
        match SocketAddr::from_str(&addr_str) {
            Ok(addr) => println!("Success: '{}' -> {:?}", addr_str, addr),
            Err(e) => println!("Error: '{}' -> {:?}", addr_str, e),
        }
    }
}