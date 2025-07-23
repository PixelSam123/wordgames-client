use std::{
    io,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::egui;
use tungstenite::stream::MaybeTlsStream;

/** (`send_message_tx`, `recv_message_rx`, `shutdown_tx`) */
pub type ChannelWebsocket = (Sender<String>, Receiver<Result<String, String>>, Sender<()>);

pub fn get_websocket_connection(url: &str, ctx: egui::Context) -> Result<ChannelWebsocket, String> {
    let (mut socket, _) = tungstenite::connect(url).map_err(|err| err.to_string())?;

    match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?,
        MaybeTlsStream::NativeTls(stream) => stream
            .get_ref()
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?,
        _ => (),
    }

    let (recv_message_tx, recv_message_rx) = mpsc::channel();
    let (send_message_tx, send_message_rx) = mpsc::channel();
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut repaint_counter = 0;

        loop {
            // Check for shutdown signal
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            if let Ok(message) = send_message_rx.try_recv() {
                if let Err(err) = socket.send(tungstenite::Message::Text(
                    tungstenite::Utf8Bytes::from(message),
                )) {
                    let _ = recv_message_tx.send(Err(err.to_string()));
                    ctx.request_repaint();
                    break;
                }
            }

            match socket.read() {
                Ok(message) => {
                    let _ = recv_message_tx.send(Ok(message.to_string()));
                    ctx.request_repaint(); // Immediate repaint for new messages
                }
                Err(tungstenite::Error::Io(ref err)) if err.kind() == io::ErrorKind::WouldBlock => {
                    // This is expected for non-blocking sockets, continue
                }
                Err(err) => {
                    let _ = recv_message_tx.send(Err(err.to_string()));
                    ctx.request_repaint();
                    break;
                }
            }

            // 30 FPS message loop, but repaint UI every 2nd iteration (15 FPS)
            repaint_counter += 1;
            if repaint_counter >= 2 {
                repaint_counter = 0;
                ctx.request_repaint();
            }

            thread::sleep(Duration::from_secs_f64(1.0 / 30.0));
        }
    });

    Ok((send_message_tx, recv_message_rx, shutdown_tx))
}
