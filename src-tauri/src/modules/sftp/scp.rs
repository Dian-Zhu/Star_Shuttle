use russh::Channel;
use std::collections::VecDeque;

pub struct ScpChannel {
    pub channel: Channel<russh::client::Msg>,
    buffer: VecDeque<u8>,
}

impl ScpChannel {
    pub fn new(channel: Channel<russh::client::Msg>) -> Self {
        Self {
            channel,
            buffer: VecDeque::new(),
        }
    }

    pub async fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.channel.data(data).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn read_more(&mut self) -> Result<(), String> {
        while self.buffer.is_empty() {
            let msg = self.channel.wait().await.ok_or("SCP channel closed")?;
            match msg {
                russh::ChannelMsg::Data { data } => {
                    for b in data.as_ref().iter().copied() {
                        self.buffer.push_back(b);
                    }
                }
                russh::ChannelMsg::ExtendedData { data, .. } => {
                    for b in data.as_ref().iter().copied() {
                        self.buffer.push_back(b);
                    }
                }
                russh::ChannelMsg::Eof => return Err("SCP channel EOF".to_string()),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn read_u8(&mut self) -> Result<u8, String> {
        self.read_more().await?;
        self.buffer
            .pop_front()
            .ok_or("SCP buffer underflow".to_string())
    }

    pub async fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, String> {
        while self.buffer.len() < n {
            self.read_more().await?;
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(
                self.buffer
                    .pop_front()
                    .ok_or("SCP buffer underflow".to_string())?,
            );
        }
        Ok(out)
    }

    pub async fn read_line(&mut self) -> Result<String, String> {
        let mut bytes = Vec::new();
        loop {
            let b = self.read_u8().await?;
            if b == b'\n' {
                break;
            }
            bytes.push(b);
        }
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }
}

pub fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

pub fn split_remote_path(path: &str) -> (String, String) {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return (".".to_string(), "".to_string());
    }
    if let Some(idx) = trimmed.rfind('/') {
        let (dir, rest) = trimmed.split_at(idx);
        let name = rest.trim_start_matches('/');
        let dir = if dir.is_empty() { "/" } else { dir };
        return (dir.to_string(), name.to_string());
    }
    (".".to_string(), trimmed.to_string())
}

pub async fn scp_read_ack(io: &mut ScpChannel) -> Result<(), String> {
    let code = io.read_u8().await?;
    match code {
        0 => Ok(()),
        1 | 2 => {
            let msg = io
                .read_line()
                .await
                .unwrap_or_else(|_| "SCP error".to_string());
            Err(msg)
        }
        other => Err(format!("SCP invalid ack: {}", other)),
    }
}
