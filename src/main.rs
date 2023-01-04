use std::{
    collections::HashSet,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::{
    egui,
    epaint::{Color32, Vec2},
};
use tungstenite::stream::MaybeTlsStream;

const APP_ZOOM: f32 = 1.071_428_5;

fn main() {
    eframe::run_native(
        "Wordgames Client",
        eframe::NativeOptions {
            initial_window_size: Some(Vec2::new(500.0, 600.0)),
            ..Default::default()
        },
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

            Box::<WordgamesClient>::default()
        }),
    );
}

type ChannelWebsocket = (Sender<String>, Receiver<Result<String, String>>);

fn connect(url: &str, ctx: egui::Context) -> Result<ChannelWebsocket, String> {
    let (mut socket, _) =
        tungstenite::connect(format!("ws://{url}")).map_err(|err| err.to_string())?;

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
    server_url: String,
    err_texts: HashSet<String>,
    messages: Vec<String>,
    websocket: Option<ChannelWebsocket>,
    message_to_send: String,
}

impl WordgamesClient {
    fn ws_result_received(&mut self, result: Result<String, String>) {
        match result {
            Ok(message) => self.messages.push(message),
            Err(err) => {
                self.err_texts.insert(err);
            }
        }
    }

    fn connect_button_clicked(&mut self, ctx: &egui::Context) {
        match connect(&self.server_url, ctx.clone()) {
            Ok(websocket) => self.websocket = Some(websocket),
            Err(err) => {
                self.err_texts.insert(err);
            }
        }
    }

    fn message_field_submitted(&mut self, message_field: &egui::Response) {
        if let Some((sender, _)) = &self.websocket {
            if !self.message_to_send.is_empty() {
                if let Err(err) = sender.send(self.message_to_send.clone()) {
                    self.err_texts.insert(err.to_string());
                }
            }
        }

        self.message_to_send = String::new();
        message_field.request_focus();
    }

    fn close_err_button_clicked(&mut self, err_text: &str) {
        self.err_texts.remove(err_text);
    }
}

impl eframe::App for WordgamesClient {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // fetch message and errors from reader thread
        if let Some((_, receiver)) = &self.websocket {
            if let Ok(result) = receiver.try_recv() {
                self.ws_result_received(result);
            }
        }

        // UI
        egui::CentralPanel::default().show(ctx, |ui| {
            for err_text in self.err_texts.clone() {
                egui::Window::new("Error")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(&err_text);
                        if ui.button("Close").clicked() {
                            self.close_err_button_clicked(&err_text);
                        }
                    });
            }

            ui.add_enabled_ui(self.websocket.is_none(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Server URL:");
                    ui.centered_and_justified(|ui| {
                        ui.text_edit_singleline(&mut self.server_url);
                    });
                });
                ui.vertical_centered_justified(|ui| {
                    if ui.button("Connect").clicked() {
                        self.connect_button_clicked(ctx);
                    }
                });
            });

            ui.heading("Messages: ");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, true])
                .max_width(f32::INFINITY)
                .max_height(ui.available_height() - 16.0)
                .show(ui, |ui| {
                    for message in &self.messages {
                        ui.label(message);
                    }
                });
        });

        egui::TopBottomPanel::bottom("bottom_panel")
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Message:");

                    ui.centered_and_justified(|ui| {
                        let message_field = ui.text_edit_singleline(&mut self.message_to_send);
                        if message_field.lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                            self.message_field_submitted(&message_field);
                        }
                    });
                });
            });
    }
}
