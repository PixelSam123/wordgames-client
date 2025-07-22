use eframe::{
    egui::{CornerRadius, Margin, Stroke, Style, TextStyle},
    epaint::{Color32, FontId, Vec2},
};

pub fn create_app_style() -> Style {
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

    app_style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.5, Color32::from_gray(60));
    app_style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::from_gray(150));
    app_style.visuals.widgets.active.bg_stroke = Stroke::new(1.5, Color32::from_gray(255));
    app_style.visuals.widgets.open.bg_stroke = Stroke::new(1.5, Color32::from_gray(60));

    app_style.visuals.widgets.hovered.expansion = 0.75;
    app_style.visuals.widgets.active.expansion = 0.75;

    app_style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.5, Color32::from_gray(190));
    app_style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.5, Color32::from_gray(220));
    app_style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::from_gray(250));
    app_style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, Color32::from_gray(255));
    app_style.visuals.widgets.open.fg_stroke = Stroke::new(1.5, Color32::from_gray(220));

    app_style.visuals.selection.stroke = Stroke::new(1.5, Color32::from_rgb(192, 222, 255));

    app_style
}
