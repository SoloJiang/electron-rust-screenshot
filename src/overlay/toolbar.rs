use crate::core::editor::{EditorState, Tool};
use egui::{RichText, Ui};

pub fn draw_toolbar(ui: &mut Ui, editor: &mut EditorState) {
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
        if ui.button("Undo").clicked() {
            editor.undo();
        }
        if ui.button("Save").clicked() {
            // Save triggered via engine in app layer
        }
    });
}
