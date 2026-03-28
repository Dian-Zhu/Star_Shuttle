pub(crate) struct TelnetConnection {
    pub read: tokio::net::tcp::OwnedReadHalf,
    pub write: tokio::net::tcp::OwnedWriteHalf,
}

pub(crate) fn telnet_process_incoming(input: &[u8], display: &mut Vec<u8>, replies: &mut Vec<u8>) {
    const IAC: u8 = 255;
    const DONT: u8 = 254;
    const DO: u8 = 253;
    const WONT: u8 = 252;
    const WILL: u8 = 251;
    const SB: u8 = 250;
    const SE: u8 = 240;

    let mut i = 0usize;
    while i < input.len() {
        if input[i] != IAC {
            display.push(input[i]);
            i += 1;
            continue;
        }

        if i + 1 >= input.len() {
            break;
        }

        let cmd = input[i + 1];
        if cmd == IAC {
            display.push(IAC);
            i += 2;
            continue;
        }

        if cmd == SB {
            i += 2;
            while i + 1 < input.len() {
                if input[i] == IAC && input[i + 1] == SE {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        if cmd == DO || cmd == DONT || cmd == WILL || cmd == WONT {
            if i + 2 >= input.len() {
                break;
            }
            let opt = input[i + 2];
            match cmd {
                DO | DONT => replies.extend_from_slice(&[IAC, WONT, opt]),
                WILL | WONT => replies.extend_from_slice(&[IAC, DONT, opt]),
                _ => {}
            }
            i += 3;
            continue;
        }

        i += 2;
    }
}
