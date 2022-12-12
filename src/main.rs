use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::{egui, epaint::Color32};
use tungstenite::stream::MaybeTlsStream;

const APP_ZOOM: f32 = 1.071_428_5;

fn main() {
    eframe::run_native(
        "Wordgames Client",
        eframe::NativeOptions::default(),
        Box::new(|creation_ctx| {
            let os_zoom_level = creation_ctx
                .integration_info
                .native_pixels_per_point
                .unwrap_or(1.0);
            creation_ctx
                .egui_ctx
                .set_pixels_per_point(os_zoom_level * APP_ZOOM);

            let mut app_visuals = egui::Visuals::dark();

            app_visuals.widgets.noninteractive.fg_stroke =
                egui::Stroke::new(1.0, Color32::from_gray(180));
            app_visuals.widgets.inactive.fg_stroke =
                egui::Stroke::new(1.0, Color32::from_gray(210));
            app_visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::from_gray(240));
            app_visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::from_gray(255));
            app_visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, Color32::from_gray(210));

            creation_ctx.egui_ctx.set_visuals(app_visuals);

            Box::new(WordgamesClient::default())
        }),
    );
}

fn connect(
    ctx: egui::Context,
) -> Result<(Sender<String>, Receiver<Result<String, String>>), String> {
    let (mut socket, _) =
        tungstenite::connect("ws://localhost:8080/ws/monka").map_err(|err| err.to_string())?;

    if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
        stream
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
    }

    let (to_main_thread_tx, to_main_thread_rx) = mpsc::channel();
    let (from_main_thread_tx, from_main_thread_rx) = mpsc::channel();

    thread::spawn(move || loop {
        if let Ok(message) = from_main_thread_rx.try_recv() {
            if let Err(err) = socket.write_message(tungstenite::Message::Text(message)) {
                to_main_thread_tx.send(Err(err.to_string())).unwrap();
                ctx.request_repaint();
            }
        }

        if let Ok(message) = socket.read_message() {
            to_main_thread_tx.send(Ok(message.to_string())).unwrap();
            ctx.request_repaint();
        }

        // approx. 60FPS loop
        thread::sleep(Duration::from_secs_f64(1.0 / 60.0));
    });

    Ok((from_main_thread_tx, to_main_thread_rx))
}

#[derive(Default)]
struct WordgamesClient {
    err_text: Option<String>,
    messages: Vec<String>,
    ws_sender: Option<Sender<String>>,
    ws_receiver: Option<Receiver<Result<String, String>>>,
    message_to_send: String,
}

impl WordgamesClient {
    fn ws_result_received(&mut self, result: Result<String, String>) {
        match result {
            Ok(message) => self.messages.push(message),
            Err(err) => self.err_text = Some(err),
        }
    }

    fn connect_button_clicked(&mut self, ctx: &egui::Context) {
        match connect(ctx.clone()) {
            Ok((ws_sender, ws_receiver)) => {
                self.ws_sender = Some(ws_sender);
                self.ws_receiver = Some(ws_receiver);
            }
            Err(err) => self.err_text = Some(err),
        }
    }

    fn message_field_submitted(&mut self) {
        if let Some(ws_sender) = &self.ws_sender {
            if let Err(err) = ws_sender.send(self.message_to_send.clone()) {
                self.err_text = Some(err.to_string());
            }
        }

        self.message_to_send = String::new();
    }
}

impl eframe::App for WordgamesClient {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // fetch message and errors from reader thread
        if let Some(ws_receiver) = &self.ws_receiver {
            if let Ok(result) = ws_receiver.try_recv() {
                self.ws_result_received(result);
            }
        }

        // UI
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("{:?}", self.err_text));

            ui.add_enabled_ui(self.ws_receiver.is_none(), |ui| {
                if ui.button("Connect to server").clicked() {
                    self.connect_button_clicked(ctx);
                }
            });

            if ui
                .text_edit_singleline(&mut self.message_to_send)
                .lost_focus()
                && ui.input().key_pressed(egui::Key::Enter)
            {
                self.message_field_submitted();
            }

            ui.heading("Messages: ");
            self.messages.iter().for_each(|message| {
                ui.label(message);
            });
        });
    }
}
