// hide terminal in --release build for Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::{
    egui::{
        style::Margin, CentralPanel, Context, Frame, Key, Response, RichText, ScrollArea, Stroke,
        Style, TextStyle, TopBottomPanel, Window,
    },
    epaint::{Color32, FontId, Rounding, Shadow, Vec2},
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
            let mut app_style = Style::default();

            app_style.spacing.item_spacing = Vec2::new(12.0, 6.0);
            app_style.spacing.button_padding = Vec2::new(6.0, 3.0);
            app_style.spacing.window_margin = Margin::same(12.0);
            app_style.spacing.menu_margin = Margin::same(12.0);

            app_style
                .text_styles
                .insert(TextStyle::Small, FontId::proportional(11.0));
            app_style
                .text_styles
                .insert(TextStyle::Body, FontId::proportional(14.0));
            app_style
                .text_styles
                .insert(TextStyle::Monospace, FontId::monospace(14.0));
            app_style
                .text_styles
                .insert(TextStyle::Button, FontId::proportional(14.0));
            app_style
                .text_styles
                .insert(TextStyle::Heading, FontId::proportional(20.0));

            app_style.visuals.window_stroke = Stroke::new(1.5, Color32::from_gray(60));
            app_style.visuals.window_rounding = Rounding::same(6.0);
            app_style.visuals.window_shadow = Shadow::small_dark();

            app_style.visuals.widgets.noninteractive.rounding = Rounding::same(3.0);
            app_style.visuals.widgets.inactive.rounding = Rounding::same(3.0);
            app_style.visuals.widgets.hovered.rounding = Rounding::same(3.0);
            app_style.visuals.widgets.active.rounding = Rounding::same(3.0);
            app_style.visuals.widgets.open.rounding = Rounding::same(3.0);

            app_style.visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.5, Color32::from_gray(60));
            app_style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::from_gray(150));
            app_style.visuals.widgets.active.bg_stroke = Stroke::new(1.5, Color32::from_gray(255));
            app_style.visuals.widgets.open.bg_stroke = Stroke::new(1.5, Color32::from_gray(60));

            app_style.visuals.widgets.hovered.expansion = 0.75;
            app_style.visuals.widgets.active.expansion = 0.75;

            app_style.visuals.widgets.noninteractive.fg_stroke =
                Stroke::new(1.5, Color32::from_gray(190));
            app_style.visuals.widgets.inactive.fg_stroke =
                Stroke::new(1.5, Color32::from_gray(220));
            app_style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::from_gray(250));
            app_style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::from_gray(255));
            app_style.visuals.widgets.open.fg_stroke = Stroke::new(1.5, Color32::from_gray(220));

            app_style.visuals.selection.stroke = Stroke::new(1.5, Color32::from_rgb(192, 222, 255));

            creation_ctx.egui_ctx.set_style(app_style);

            // TIMER HACK: Re-render UI every second
            let app_ctx = creation_ctx.egui_ctx.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(1));
                app_ctx.request_repaint();
            });

            Box::<WordgamesClient>::default()
        }),
    );
}

type ChannelWebsocket = (Sender<String>, Receiver<Result<String, String>>);

fn connect(url: &str, ctx: Context) -> Result<ChannelWebsocket, String> {
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
    timer_finish_time: Option<OffsetDateTime>,
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
                    self.timer_finish_time = None;
                    self.status_text = "Waiting Round Start!".to_owned();
                    self.word_box = String::new();
                }
                ServerMessage::FinishedRoundInfo {
                    word_answer,
                    to_next_round_time,
                } => {
                    self.timer_finish_time = Some(
                        OffsetDateTime::parse(&to_next_round_time, &Iso8601::DEFAULT).unwrap(),
                    );
                    self.status_text = "Time's up! The answer is:".to_owned();
                    self.word_box = word_answer;
                }
                ServerMessage::OngoingRoundInfo {
                    word_to_guess,
                    round_finish_time,
                } => {
                    self.timer_finish_time =
                        Some(OffsetDateTime::parse(&round_finish_time, &Iso8601::DEFAULT).unwrap());
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

        TopBottomPanel::bottom("bottom_panel")
            .frame(Frame {
                inner_margin: Margin::same(12.0),
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
                    left: 12.0,
                    right: 12.0,
                    top: 12.0,
                    bottom: 0.0,
                },
                ..Frame::central_panel(&ctx.style())
            })
            .show(ctx, |ui| {
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

                ui.label(&format!(
                    "{} {}",
                    self.status_text,
                    self.timer_finish_time.map_or(String::new(), |time| format!(
                        "{} seconds",
                        (time - OffsetDateTime::now_utc()).as_seconds_f32().round()
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
