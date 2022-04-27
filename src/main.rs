#![allow(clippy::type_complexity)]

use assets::{load_assets, GemAssets};
use bevy::{app::AppExit, gltf::Gltf, prelude::*};
use bevy_egui::{
    egui::{self, FontId, RichText},
    EguiContext, EguiPlugin,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_match3::{prelude::*, Match3Config};
use heron::PhysicsPlugin;
use strum::{EnumIter, IntoEnumIterator};

mod assets;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Msaa { samples: 4 })
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 3.0,
        })
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugin(EguiPlugin)
        .add_plugin(WorldInspectorPlugin::default())
        .add_plugin(PhysicsPlugin::default())
        .insert_resource(Match3Config {
            gem_types: 8,
            board_dimensions: UVec2::splat(8),
        })
        .add_plugin(Match3Plugin)
        .add_state(GameState::MainMenu)
        .add_startup_system(setup)
        .add_startup_system(load_assets)
        .add_system(apply_material)
        .add_system_set(SystemSet::on_enter(GameState::MainMenu))
        .add_system_set(SystemSet::on_update(GameState::MainMenu).with_system(main_menu))
        .add_system_set(SystemSet::on_exit(GameState::MainMenu))
        .add_system_set(SystemSet::on_enter(GameState::Game).with_system(spawn_board))
        .add_system_set(SystemSet::on_update(GameState::Game).with_system(gem_events))
        .add_system_set(SystemSet::on_exit(GameState::Game))
        .run()
}

fn setup(mut commands: Commands) {
    let mut camera = OrthographicCameraBundle::new_3d();
    camera.transform = Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y);
    commands.spawn_bundle(camera);
}

fn main_menu(
    mut egui_ctx: ResMut<EguiContext>,
    mut state: ResMut<State<GameState>>,
    mut events: EventWriter<AppExit>,
) {
    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        ui.set_min_width(200.0);
        ui.with_layout(
            egui::Layout::default().with_cross_align(egui::Align::Center),
            |ui| {
                ui.heading(RichText::new("PUZZLE QUEST 2").font(FontId::monospace(100.0)));
                if ui
                    .button(RichText::new("Start").font(FontId::monospace(50.0)))
                    .clicked()
                {
                    state.set(GameState::Game).unwrap();
                }
                if ui
                    .button(RichText::new("Exit").font(FontId::monospace(50.0)))
                    .clicked()
                {
                    events.send(AppExit);
                }
            },
        );
    });
}

fn spawn_board(
    mut commands: Commands,
    assets: Res<GemAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    board: Res<Board>,
) {
    let size = 0.2;
    let left = -(size * 4.0) + (size / 2.0);
    let bottom = left;
    board.iter().for_each(|(pos, typ)| {
        spawn_gem(
            &mut commands,
            Vec3::new(
                left + pos.x as f32 * size,
                bottom + pos.y as f32 * size,
                0.0,
            ),
            (*typ as u8).into(),
            &gltf_assets,
            &assets,
        );
    });
}

fn gem_events(mut events: ResMut<BoardEvents>) {
    if let Ok(event) = events.pop() {
        match event {
            BoardEvent::Swapped(_, _) => todo!(),
            BoardEvent::FailedSwap(_, _) => todo!(),
            BoardEvent::Dropped(_) => todo!(),
            BoardEvent::Popped(_) => todo!(),
            BoardEvent::Spawned(_) => todo!(),
            BoardEvent::Matched(_) => todo!(),
        }
    }
}

fn spawn_gem(
    commands: &mut Commands,
    pos: Vec3,
    typ: GemType,
    gltf_assets: &Res<Assets<Gltf>>,
    assets: &Res<GemAssets>,
) {
    commands
        .spawn_bundle((
            Transform::from_translation(pos),
            GlobalTransform::default(),
            typ,
        ))
        .with_children(|parent| {
            parent.spawn_scene(
                gltf_assets
                    .get(assets.meshes.get(&typ.into()).unwrap())
                    .unwrap()
                    .scenes[0]
                    .clone(),
            );
        });
}

fn apply_material(
    assets: Res<GemAssets>,
    gems: Query<(&GemType, &Children), Added<GemType>>,
    mut children_query: Query<
        (Option<&mut Handle<StandardMaterial>>, Option<&Children>),
        With<Parent>,
    >,
    mut to_check: Local<Vec<Entity>>,
) {
    for (typ, children) in gems.iter() {
        to_check.extend(children.iter().copied());
        while let Some(child) = to_check.pop() {
            if let Ok((material, children)) = children_query.get_mut(child) {
                if let Some(mut mat) = material {
                    *mat = assets.materials[*typ as usize].clone_weak();
                }
                to_check.extend(children.iter().flat_map(|children| children.iter()));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum GameState {
    MainMenu,
    Game,
}

#[repr(u8)]
#[derive(Component, Clone, Copy, EnumIter)]
enum GemType {
    Ruby,
    Emerald,
    Sapphire,
    Topaz,
    Diamond,
    Amethyst,
    Skull,
    Equipment,
}

impl From<u8> for GemType {
    fn from(val: u8) -> Self {
        GemType::iter()
            .enumerate()
            .find(|(i, _)| *i == val as usize)
            .unwrap()
            .1
    }
}
