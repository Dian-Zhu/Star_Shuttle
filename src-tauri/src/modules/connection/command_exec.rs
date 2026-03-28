use crate::modules::connection::{ConnectionError, SshConnection};
use tokio::runtime::Runtime;

pub fn exec_command(
    runtime: &Runtime,
    ssh_connection: SshConnection,
    command: &str,
) -> Result<String, ConnectionError> {
    let command = command.to_string();

    runtime.block_on(async move {
        let mut channel = {
            let handle = ssh_connection.handle.lock().await;
            handle.channel_open_session().await.map_err(|e| {
                ConnectionError::SshError(format!("Failed to open channel: {:?}", e))
            })?
        };

        channel
            .exec(true, command.as_bytes().to_vec())
            .await
            .map_err(|e| {
                ConnectionError::SshError(format!("Failed to execute command: {:?}", e))
            })?;

        let mut output = String::new();
        while let Some(msg) = channel.wait().await {
            match msg {
                russh::ChannelMsg::Data { ref data } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                russh::ChannelMsg::ExtendedData { ref data, .. } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                russh::ChannelMsg::Eof => {
                    break;
                }
                _ => {}
            }
        }
        channel.close().await.ok();
        Ok(output)
    })
}
