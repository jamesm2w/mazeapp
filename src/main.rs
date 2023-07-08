extern crate maze;

pub mod exercise_1;
pub mod gui;

fn main() {
    let mut options = eframe::NativeOptions::default();
    options.initial_window_size = Some(eframe::egui::Vec2 { x: 800.0, y: 600.0 });
    options.vsync = false;
    eframe::run_native(
        "Maze App",
        options,
        Box::new(|_cc| Box::new(gui::create_app())),
    );
}
