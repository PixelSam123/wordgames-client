use std::{
    net::TcpStream,
    sync::{Arc, Mutex},
    thread,
};

use eframe::{egui, epaint::Color32};
use tungstenite::{stream::MaybeTlsStream, WebSocket};

const APP_ZOOM: f32 = 1.071_428_5 * 1.25;

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

            let app = WordgamesClient::default();

            Box::new(app)
        }),
    );
}

fn connect(
    ctx: egui::Context,
    messages: Arc<Mutex<Vec<String>>>,
) -> Result<Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>, String> {
    let (socket, _) =
        tungstenite::connect("ws://localhost:8080/ws/monka").map_err(|err| err.to_string())?;
    let socket = Arc::new(Mutex::new(socket));

    let reader_socket = socket.clone();
    thread::spawn(move || loop {
        let msg = reader_socket.lock().unwrap().read_message().unwrap();
        messages.lock().unwrap().push(msg.to_string());

        ctx.request_repaint();
    });

    Ok(socket)
}

#[derive(Default)]
struct WordgamesClient {
    ws: Option<Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>>,
    err_text: Option<String>,
    messages: Arc<Mutex<Vec<String>>>,
    message_to_send: String,
}

impl WordgamesClient {
    fn connect_button_clicked(&mut self, ctx: &egui::Context) {
        match connect(ctx.clone(), self.messages.clone()) {
            Ok(ws) => self.ws = Some(ws),
            Err(err) => self.err_text = Some(err),
        }
    }

    fn message_field_submitted(&mut self) {
        if let Some(ws) = &self.ws {
            // Deadlock here
            ws.lock().unwrap().write_message(tungstenite::Message::Text(self.message_to_send.clone())).unwrap();
        }

        self.message_to_send = String::new();
    }
}

impl eframe::App for WordgamesClient {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("{:?}", self.err_text));

            ui.add_enabled_ui(self.ws.is_none(), |ui| {
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
            self.messages.lock().unwrap().iter().for_each(|message| {
                ui.label(message);
            });
        });
    }
}
