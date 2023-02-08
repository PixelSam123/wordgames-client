use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::{
    egui::{
        CentralPanel, Context, Key, Response, RichText, ScrollArea, Stroke, TopBottomPanel,
        Window, TextStyle,
    },
    epaint::{Color32, Vec2, Rounding, Shadow, FontId},
};
use serde::Deserialize;
use time::{format_description::well_known::Iso8601, OffsetDateTime};
use tungstenite::stream::MaybeTlsStream;

const APP_NAME: &str = "Wordgames Client";

fn main() {
    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions {
            initial_window_size: Some(Vec2::new(500.0, 600.0)),
            min_window_size: Some(Vec2::new(300.0, 300.0)),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|creation_ctx| {
            let mut app_style = creation_ctx.egui_ctx.style().as_ref().clone();

            app_style.spacing.item_spacing = Vec2::new(12.0, 6.0);
            app_style.spacing.button_padding = Vec2::new(6.0, 3.0);

            app_style.text_styles.insert(TextStyle::Small, FontId::proportional(10.0));
            app_style.text_styles.insert(TextStyle::Body, FontId::proportional(13.0));
            app_style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
            app_style.text_styles.insert(TextStyle::Button, FontId::proportional(13.0));
            app_style.text_styles.insert(TextStyle::Heading, FontId::proportional(19.0));

            app_style.visuals.window_stroke = Stroke::new(1.5, Color32::from_gray(60));
            app_style.visuals.window_rounding = Rounding::none();
            app_style.visuals.window_shadow = Shadow::small_dark();

            app_style.visuals.widgets.noninteractive.rounding = Rounding::none();
            app_style.visuals.widgets.inactive.rounding = Rounding::none();
            app_style.visuals.widgets.hovered.rounding = Rounding::none();
            app_style.visuals.widgets.active.rounding = Rounding::none();
            app_style.visuals.widgets.open.rounding = Rounding::none();

            app_style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.5, Color32::from_gray(60));
            app_style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::from_gray(150));
            app_style.visuals.widgets.active.bg_stroke = Stroke::new(1.5, Color32::from_gray(255));
            app_style.visuals.widgets.open.bg_stroke = Stroke::new(1.5, Color32::from_gray(60));

            app_style.visuals.widgets.hovered.expansion = 0.75;
            app_style.visuals.widgets.active.expansion = 0.75;

            app_style.visuals.widgets.noninteractive.fg_stroke =
                Stroke::new(1.5, Color32::from_gray(190));
            app_style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.5, Color32::from_gray(220));
            app_style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::from_gray(250));
            app_style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::from_gray(255));
            app_style.visuals.widgets.open.fg_stroke = Stroke::new(1.5, Color32::from_gray(220));

            app_style.visuals.selection.stroke = Stroke::new(1.5, Color32::from_rgb(192, 222, 255));

            creation_ctx.egui_ctx.set_style(app_style);

            Box::<WordgamesClient>::default()
        }),
    );
}

type ChannelWebsocket = (Sender<String>, Receiver<Result<String, String>>);

fn connect(url: &str, ctx: Context) -> Result<ChannelWebsocket, String> {
    let (mut socket, _) = tungstenite::connect(url).map_err(|err| err.to_string())?;

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

#[derive(Deserialize)]
#[serde(tag = "type", content = "content")]
enum ServerMessage {
    ChatMessage(String),
    FinishedGame,
    FinishedRoundInfo {
        word_answer: String,
        to_next_round_time: String,
    },
    OngoingRoundInfo {
        word_to_guess: String,
        round_finish_time: String,
    },
}

#[derive(Default)]
struct WordgamesClient {
    err_texts: Vec<String>,
    messages: Vec<String>,
    message_to_send: String,
    server_url: String,
    status_text: String,
    timer_text: String,
    websocket: Option<ChannelWebsocket>,
    word_box: String,
}

impl WordgamesClient {
    fn ws_result_received(&mut self, result: Result<String, String>) {
        match result {
            Ok(message) => match serde_json::from_str::<ServerMessage>(&message).unwrap() {
                ServerMessage::ChatMessage(message) => {
                    self.messages.push(message);
                }
                ServerMessage::FinishedGame => {
                    self.timer_text = String::new();
                    self.status_text = "Waiting Round Start!".to_owned();
                    self.word_box = String::new();
                }
                ServerMessage::FinishedRoundInfo {
                    word_answer,
                    to_next_round_time,
                } => {
                    let next_round_time =
                        OffsetDateTime::parse(&to_next_round_time, &Iso8601::DEFAULT).unwrap();
                    self.timer_text = format!(
                        "next round starts in {} seconds",
                        (next_round_time - OffsetDateTime::now_utc())
                            .as_seconds_f32()
                            .round() as i32
                    );
                    self.status_text = "Time's up! The answer is:".to_owned();
                    self.word_box = word_answer;
                }
                ServerMessage::OngoingRoundInfo {
                    word_to_guess,
                    round_finish_time,
                } => {
                    let finish_time =
                        OffsetDateTime::parse(&round_finish_time, &Iso8601::DEFAULT).unwrap();
                    self.timer_text = format!(
                        "time is {} seconds",
                        (finish_time - OffsetDateTime::now_utc())
                            .as_seconds_f32()
                            .round() as i32
                    );
                    self.status_text = "Please guess:".to_owned();
                    self.word_box = word_to_guess;
                }
            },
            Err(err) => {
                self.err_texts.push(err);
            }
        }
    }

    fn connect_button_clicked(&mut self, ctx: &Context) {
        match connect(&self.server_url, ctx.clone()) {
            Ok(websocket) => self.websocket = Some(websocket),
            Err(err) => {
                self.err_texts.push(err);
            }
        }
    }

    fn disconnect_button_clicked(&mut self) {
        self.websocket = None;
    }

    fn message_field_submitted(&mut self, message_field: &Response) {
        if let Some((sender, _)) = &self.websocket {
            if !self.message_to_send.is_empty() {
                if let Err(err) = sender.send(self.message_to_send.clone()) {
                    self.err_texts.push(err.to_string());
                }
            }
        }

        self.message_to_send = String::new();
        message_field.request_focus();
    }

    fn close_err_button_clicked(&mut self, idx: usize) {
        self.err_texts.remove(idx);
    }
}

impl eframe::App for WordgamesClient {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        // fetch message and errors from reader thread
        if let Some((_, receiver)) = &self.websocket {
            if let Ok(result) = receiver.try_recv() {
                self.ws_result_received(result);
            }
        }

        // UI
        for (idx, err_text) in self.err_texts.clone().iter().enumerate() {
            Window::new(format!("Error {}", idx + 1))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(err_text);
                    if ui.button("Close").clicked() {
                        self.close_err_button_clicked(idx);
                    }
                });
        }

        CentralPanel::default().show(ctx, |ui| {
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
            ui.add_enabled_ui(self.websocket.is_some(), |ui| {
                ui.vertical_centered_justified(|ui| {
                    if ui.button("Disconnect").clicked() {
                        self.disconnect_button_clicked();
                    }
                });
            });

            ui.label(&format!("{}, {}", self.status_text, self.timer_text));
            ui.label(RichText::new(&self.word_box).code().size(32.0));

            ui.heading("Messages: ");
            ScrollArea::vertical()
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

        TopBottomPanel::bottom("bottom_panel")
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Message:");

                    ui.centered_and_justified(|ui| {
                        let message_field = ui.text_edit_singleline(&mut self.message_to_send);
                        if message_field.lost_focus() && ui.input().key_pressed(Key::Enter) {
                            self.message_field_submitted(&message_field);
                        }
                    });
                });
            });
    }
}
