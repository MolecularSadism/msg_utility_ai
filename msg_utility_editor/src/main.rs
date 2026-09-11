use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui;

mod document;
mod graph;
mod io;
mod simulation;
mod ui;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Utility AI Editor".into(),
                resolution: bevy::window::WindowResolution::new(1400, 900),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(bevy_egui::EguiPlugin::default())
        .init_resource::<document::EditorDocument>()
        .add_plugins(ui::UiPlugin)
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
