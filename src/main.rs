use eframe::{egui, epaint::Color32};

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

#[derive(Default)]
struct WordgamesClient;

impl eframe::App for WordgamesClient {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Hello egui from eframe!");
        });
    }
}
