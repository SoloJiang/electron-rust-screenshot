use crate::core::editor::{EditorState, Tool};
use egui::{Color32, RichText, Ui};

pub fn draw_toolbar(ui: &mut Ui, editor: &mut EditorState) -> bool {
    let mut save_clicked = false;
    ui.horizontal(|ui| {
        let tools = [
            ("Rect", Tool::Rect),
            ("Ellipse", Tool::Ellipse),
            ("Arrow", Tool::Arrow),
            ("Brush", Tool::Brush),
            ("Mosaic", Tool::Mosaic),
            ("Text", Tool::Text),
        ];
        for (label, tool) in tools {
            let button =
                ui.selectable_label(editor.active_tool == tool, RichText::new(label).size(14.0));
            if button.clicked() {
                editor.active_tool = tool;
            }
        }

        ui.separator();

        let mut srgba = Color32::from_rgba_premultiplied(
            editor.tool_color.r,
            editor.tool_color.g,
            editor.tool_color.b,
            editor.tool_color.a,
        );
        let color_response = egui::color_picker::color_edit_button_srgba(
            ui,
            &mut srgba,
            egui::color_picker::Alpha::Opaque,
        );
        if color_response.changed() {
            editor.tool_color =
                crate::core::types::Color::new(srgba.r(), srgba.g(), srgba.b(), srgba.a());
        }

        ui.add(
            egui::Slider::new(&mut editor.tool_size, 1.0..=20.0)
                .text("Size")
                .show_value(true),
        );

        ui.separator();
        if ui.button("Undo (Cmd+Z)").clicked() {
            editor.undo();
        }
        if ui.button("Redo (Cmd+Shift+Z)").clicked() {
            editor.redo();
        }
        if ui.button("Save (Enter)").clicked() {
            save_clicked = true;
        }
    });
    save_clicked
}
