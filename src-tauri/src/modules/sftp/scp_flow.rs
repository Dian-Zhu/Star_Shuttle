use crate::modules::connection::DefaultConnectionManager;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use super::common::{
    ensure_max_bytes, ensure_scp_upload_size, validate_scp_directory, validate_scp_file_name,
    MAX_SFTP_READ_BYTES,
};
use super::generation::{ensure_generation_current, SessionGenerationMap};
use super::scp::{scp_read_ack, shell_quote, split_remote_path, ScpChannel};
use super::ssh_bridge::open_exec_channel;

async fn ensure_scp_generation_valid(
    generations: &SessionGenerationMap,
    session_id: Uuid,
    generation: u64,
    io: &mut ScpChannel,
) -> Result<(), String> {
    match ensure_generation_current(generations.as_ref(), session_id, generation) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = io.channel.close().await;
            Err(err)
        }
    }
}

pub async fn scp_upload(
    generations: &SessionGenerationMap,
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    generation: u64,
    remote_path: String,
    content: Vec<u8>,
) -> Result<(), String> {
    ensure_scp_upload_size(content.len(), "SCP upload")?;
    let (dir, file_name) = split_remote_path(&remote_path);
    validate_scp_directory(&dir)?;
    validate_scp_file_name(&file_name)?;

    ensure_generation_current(generations.as_ref(), session_id, generation)?;

    let command = format!("scp -t -- {}", shell_quote(&dir));
    let channel = open_exec_channel(connection_manager, session_id, command).await?;
    if let Err(err) = ensure_generation_current(generations.as_ref(), session_id, generation) {
        let _ = channel.close().await;
        return Err(err);
    }

    let mut io = ScpChannel::new(channel);
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;

    let header = format!("C0644 {} {}\n", content.len(), file_name);
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
    io.write_all(header.as_bytes()).await?;
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
    scp_read_ack(&mut io).await?;
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;

    if !content.is_empty() {
        ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
        io.write_all(&content).await?;
        ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
    }
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
    io.write_all(&[0]).await?;
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
    scp_read_ack(&mut io).await?;
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;

    let _ = io.channel.close().await;
    Ok(())
}

pub async fn scp_download(
    generations: &SessionGenerationMap,
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    generation: u64,
    remote_path: String,
) -> Result<Vec<u8>, String> {
    ensure_generation_current(generations.as_ref(), session_id, generation)?;

    let command = format!("scp -f -- {}", shell_quote(&remote_path));
    let channel = open_exec_channel(connection_manager, session_id, command).await?;
    if let Err(err) = ensure_generation_current(generations.as_ref(), session_id, generation) {
        let _ = channel.close().await;
        return Err(err);
    }

    let mut io = ScpChannel::new(channel);

    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
    io.write_all(&[0]).await?;
    ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;

    loop {
        ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
        let b = io.read_u8().await?;
        ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
        match b {
            b'T' => {
                let _ = io.read_line().await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
                io.write_all(&[0]).await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
            }
            b'C' => {
                let header = io.read_line().await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
                let mut parts = header.splitn(3, ' ');
                let _mode = parts.next().ok_or("SCP missing mode")?;
                let size_str = parts.next().ok_or("SCP missing size")?;
                let _name = parts.next().ok_or("SCP missing filename")?;
                let size = size_str.parse::<usize>().map_err(|e| e.to_string())?;
                ensure_max_bytes(size, MAX_SFTP_READ_BYTES, "SCP download")?;

                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
                io.write_all(&[0]).await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
                let data = io.read_exact(size).await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
                scp_read_ack(&mut io).await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;
                io.write_all(&[0]).await?;
                ensure_scp_generation_valid(generations, session_id, generation, &mut io).await?;

                let _ = io.channel.close().await;
                return Ok(data);
            }
            0 => {}
            1 | 2 => {
                let msg = io
                    .read_line()
                    .await
                    .unwrap_or_else(|_| "SCP error".to_string());
                return Err(msg);
            }
            other => return Err(format!("SCP unexpected response byte: {}", other)),
        }
    }
}
