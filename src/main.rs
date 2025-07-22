// hide terminal in --release build for Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use eframe::{
    egui::{
        CentralPanel, Context, CornerRadius, Frame, Key, Margin, Response, RichText, ScrollArea,
        Stroke, Style, TextStyle, TopBottomPanel, ViewportBuilder, Window,
    },
    epaint::{Color32, FontId, Vec2},
};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Iso8601};
use tungstenite::stream::MaybeTlsStream;

const APP_NAME: &str = "Wordgames Client";

fn main() -> eframe::Result {
    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions {
            viewport: ViewportBuilder::default()
                .with_inner_size(Vec2::new(500.0, 600.0))
                .with_min_inner_size(Vec2::new(300.0, 300.0)),
            ..Default::default()
        },
        Box::new(|creation_ctx| {
            let mut app_style = Style::default();

            app_style.spacing.item_spacing = Vec2::new(12.0, 6.0);
            app_style.spacing.button_padding = Vec2::new(6.0, 3.0);
            app_style.spacing.window_margin = Margin::same(12);
            app_style.spacing.menu_margin = Margin::same(12);

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
            app_style.visuals.window_corner_radius = CornerRadius::same(6);

            app_style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(3);
            app_style.visuals.widgets.inactive.corner_radius = CornerRadius::same(3);
            app_style.visuals.widgets.hovered.corner_radius = CornerRadius::same(3);
            app_style.visuals.widgets.active.corner_radius = CornerRadius::same(3);
            app_style.visuals.widgets.open.corner_radius = CornerRadius::same(3);

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

            Ok(Box::<WordgamesClient>::default())
        }),
    )
}

type ChannelWebsocket = (Sender<String>, Receiver<Result<String, String>>, Sender<()>);

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
                Err(tungstenite::Error::Io(ref err))
                    if err.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    // This is expected for non-blocking sockets, continue
                }
                Err(err) => {
                    let _ = recv_message_tx.send(Err(err.to_string()));
                    ctx.request_repaint();
                    break;
                }
            }

            // 30 FPS message loop, but repaint UI every 3rd iteration (10 FPS)
            repaint_counter += 1;
            if repaint_counter >= 3 {
                repaint_counter = 0;
                ctx.request_repaint();
            }

            thread::sleep(Duration::from_secs_f64(1.0 / 30.0));
        }
    });

    Ok((send_message_tx, recv_message_rx, shutdown_tx))
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
        match connect(&self.server_url, ctx.clone()) {
            Ok(websocket) => self.websocket = Some(websocket),
            Err(err) => {
                self.err_texts.push(err);
            }
        }
    }

    fn disconnect_button_clicked(&mut self) {
        if let Some((_, _, shutdown_tx)) = &self.websocket {
            let _ = shutdown_tx.send(());
        }
        self.websocket = None;

        self.timer_finish_time = None;
        self.word_box_guide = "";
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
}

impl eframe::App for WordgamesClient<'_> {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
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
