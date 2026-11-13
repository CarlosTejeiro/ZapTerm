#![forbid(unsafe_code)]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedSender;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

use crate::error::Error;
use crate::SessionCmd;

// ---------------------------------------------------------------------------
// Port enumeration (synchronous — called from blocking context or at startup)
// ---------------------------------------------------------------------------

pub fn list_ports() -> Vec<String> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .collect()
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

pub struct SerialTransport {
    port: tokio_serial::SerialStream,
}

impl SerialTransport {
    /// Open a serial port with common 8N1 settings.
    pub fn open(device: &str, baud_rate: u32) -> Result<Self, Error> {
        let port = tokio_serial::new(device, baud_rate)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None)
            .open_native_async()?;
        Ok(Self { port })
    }

    /// Run the session. `on_close` fires once if the port fails by itself
    /// (adapter unplugged, read error) — never when the app closes the
    /// session by dropping the returned sender. Closing releases the port:
    /// both tasks end, so it can be opened again (ports are exclusive).
    pub fn start_io_loop<F, C>(self, on_data: F, on_close: C) -> UnboundedSender<SessionCmd>
    where
        F: Fn(Vec<u8>) + Send + 'static,
        C: FnOnce(crate::CloseReason) + Send + 'static,
    {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<SessionCmd>();
        let (mut read_half, mut write_half) = tokio::io::split(self.port);
        // Told by the writer when the app drops `cmd_tx`. A serial port never
        // reports EOF, so without this the reader (and the port) would live on.
        let (closed_tx, mut closed_rx) = tokio::sync::oneshot::channel::<()>();

        // Writer task
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    SessionCmd::Data(data) => {
                        if write_half.write_all(&data).await.is_err() {
                            return;
                        }
                    }
                    // Serial ports have no PTY resize concept — silently ignore.
                    SessionCmd::Resize { .. } => {}
                }
            }
            let _ = closed_tx.send(());
        });

        // Reader task
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let mut writer_gone = false;
            let reason = loop {
                let read = tokio::select! {
                    res = &mut closed_rx, if !writer_gone => {
                        // Ok: the app closed the session. Err: the writer
                        // stopped on a write error — let the read report it.
                        if res.is_ok() {
                            break None;
                        }
                        writer_gone = true;
                        continue;
                    }
                    read = read_half.read(&mut buf) => read,
                };
                match read {
                    Ok(0) => break Some(crate::CloseReason::Ended),
                    Err(_) => break Some(crate::CloseReason::Lost),
                    Ok(n) => on_data(buf[..n].to_vec()),
                }
            };
            if let Some(reason) = reason {
                on_close(reason);
            }
        });

        cmd_tx
    }
}
