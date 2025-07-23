// hide terminal in --release build for Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{
    AppCreator,
    egui::{
        Align2, CentralPanel, Context, Frame, Key, Margin, Response, RichText, ScrollArea,
        TopBottomPanel, ViewportBuilder, Window,
    },
    epaint::Vec2,
    icon_data,
};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Iso8601};

use crate::{
    style::create_app_style,
    utils::{ChannelWebsocket, get_websocket_connection},
};

mod style;
mod utils;

const APP_NAME: &str = "Wordgames Client";
const ICON: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(Vec2::new(500.0, 600.0))
            .with_min_inner_size(Vec2::new(300.0, 300.0))
            .with_icon(
                icon_data::from_png_bytes(ICON)
                    .map_err(|e| eframe::Error::AppCreation(Box::new(e)))?,
            ),
        ..Default::default()
    };

    let app_creator: AppCreator = Box::new(|creation_ctx| {
        creation_ctx.egui_ctx.set_style(create_app_style());

        let storage = creation_ctx.storage;

        Ok(Box::new(WordgamesClient {
            server_url: storage
                .and_then(|s| s.get_string("server_url"))
                .unwrap_or_else(|| "ws://localhost:3000/ws/anagram/1".to_string()),
            word_box_guide: "Waiting Round Start!",
            ..Default::default()
        }))
    });

    eframe::run_native(APP_NAME, options, app_creator)
}

#[derive(Default)]
struct WordgamesClient<'a> {
    err_texts: Vec<String>,
    messages: Vec<String>,
    message_to_send: String,
    server_url: String,
    word_box_guide: &'a str,
    timer_finish_time: Option<OffsetDateTime>,
    websocket: Option<ChannelWebsocket>,
    word_box: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "content")]
enum ServerMessage {
    ChatMessage(String),
    OngoingRoundInfo {
        word_to_guess: String,
        round_finish_time: String,
    },
    FinishedRoundInfo {
        word_answer: String,
        to_next_round_time: String,
    },
    FinishedGame,
}

impl WordgamesClient<'_> {
    fn ws_result_received(&mut self, result: Result<String, String>) {
        match result {
            Ok(message) => match serde_json::from_str::<ServerMessage>(&message)
                .unwrap_or_else(|_| ServerMessage::ChatMessage("Error parsing message".to_string()))
            {
                ServerMessage::ChatMessage(message) => {
                    self.messages.push(message);
                }
                ServerMessage::FinishedGame => {
                    self.timer_finish_time = None;
                    self.word_box_guide = "Waiting Round Start!";
                    self.word_box = String::new();
                }
                ServerMessage::FinishedRoundInfo {
                    word_answer,
                    to_next_round_time,
                } => {
                    self.timer_finish_time =
                        OffsetDateTime::parse(&to_next_round_time, &Iso8601::DEFAULT).ok();
                    self.word_box_guide = "Time's up! The answer is:";
                    self.word_box = word_answer;
                }
                ServerMessage::OngoingRoundInfo {
                    word_to_guess,
                    round_finish_time,
                } => {
                    self.timer_finish_time =
                        OffsetDateTime::parse(&round_finish_time, &Iso8601::DEFAULT).ok();
                    self.word_box_guide = "Please guess:";
                    self.word_box = word_to_guess;
                }
            },
            Err(err) => {
                self.err_texts.push(err);
            }
        }
    }

    fn connect_button_clicked(&mut self, ctx: &Context) {
        match get_websocket_connection(&self.server_url, ctx.clone()) {
            Ok(websocket) => self.websocket = Some(websocket),
            Err(err) => {
                self.err_texts.push(err);
            }
        }
    }

    fn disconnect_button_clicked(&mut self) {
        if let Some((_, _, shutdown_tx)) = &self.websocket {
            if let Err(err) = shutdown_tx.send(()) {
                self.err_texts.push(err.to_string());
                return;
            }
        }
        self.websocket = None;

        self.timer_finish_time = None;
        self.word_box_guide = "Waiting Round Start!";
        self.word_box = String::new();
    }

    fn message_field_submitted(&mut self, message_field: &Response) {
        if let Some((sender, _, _)) = &self.websocket {
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

    fn server_url_changed(&self, frame: &mut eframe::Frame) {
        // Save server URL to storage when it's modified
        if let Some(storage) = frame.storage_mut() {
            storage.set_string("server_url", self.server_url.clone());
        }
    }
}

impl eframe::App for WordgamesClient<'_> {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        // fetch message and errors from reader thread
        if let Some((_, receiver, _)) = &self.websocket {
            if let Ok(result) = receiver.try_recv() {
                self.ws_result_received(result);
            }
        }

        // UI
        for (idx, err_text) in self.err_texts.clone().iter().enumerate() {
            Window::new(format!("Error {}", idx + 1))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(err_text);
                    if ui.button("Close").clicked() {
                        self.close_err_button_clicked(idx);
                    }
                });
        }

        TopBottomPanel::bottom("bottom_panel")
            .frame(Frame {
                inner_margin: Margin::same(12),
                ..Frame::side_top_panel(&ctx.style())
            })
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Message:");

                    ui.centered_and_justified(|ui| {
                        let message_field = ui.text_edit_singleline(&mut self.message_to_send);
                        if message_field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.message_field_submitted(&message_field);
                        }
                    });
                });
            });

        CentralPanel::default()
            .frame(Frame {
                inner_margin: Margin {
                    left: 12,
                    right: 12,
                    top: 12,
                    bottom: 0,
                },
                ..Frame::central_panel(&ctx.style())
            })
            .show(ctx, |ui| {
                ui.add_enabled_ui(self.websocket.is_none(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Server URL:");
                        ui.centered_and_justified(|ui| {
                            let server_url_field = ui.text_edit_singleline(&mut self.server_url);
                            if server_url_field.changed() {
                                self.server_url_changed(frame);
                            }
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

                ui.label(format!(
                    "{} {}",
                    self.word_box_guide,
                    self.timer_finish_time.map_or(String::new(), |time| format!(
                        "{:.1} seconds",
                        (time - OffsetDateTime::now_utc()).as_seconds_f32()
                    ))
                ));
                ui.label(RichText::new(&self.word_box).code().size(32.0));

                ui.heading("Messages: ");
                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, true])
                    .max_width(f32::INFINITY)
                    .show(ui, |ui| {
                        for message in &self.messages {
                            ui.label(message);
                        }
                    });
            });
    }
}
