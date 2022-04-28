#![allow(clippy::type_complexity)]
#![feature(is_some_with)]

use std::time::Duration;

use assets::{load_assets, GemAssets};
use bevy::{app::AppExit, gltf::Gltf, prelude::*};
use bevy_egui::{
    egui::{self, FontId, RichText},
    EguiContext, EguiPlugin,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_match3::{prelude::*, Match3Config};
use bevy_mod_raycast::{DefaultRaycastingPlugin, RayCastMesh, RayCastMethod, RayCastSource};
use bevy_tweening::{lens::*, Animator, EaseFunction, Tween, TweeningPlugin, TweeningType};
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
        .add_plugin(DefaultRaycastingPlugin::<RaycastSet>::default())
        .add_plugin(TweeningPlugin)
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
        .add_system_set(
            SystemSet::on_update(GameState::Game)
                .with_system(gem_events)
                .with_system(update_raycast_with_cursor)
                .with_system(select)
                .with_system(animate_selected),
        )
        .add_system_set(SystemSet::on_exit(GameState::Game))
        .run()
}

fn setup(mut commands: Commands) {
    let mut camera = OrthographicCameraBundle::new_3d();
    camera.transform = Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y);
    commands
        .spawn_bundle(camera)
        .insert(RayCastSource::<RaycastSet>::new());
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
    let top = (size * 4.0) - (size / 2.0);
    let left = -(size * 4.0) + (size / 2.0);
    board.iter().for_each(|(pos, typ)| {
        let translation = Vec3::new(left + pos.x as f32 * size, top - pos.y as f32 * size, 0.0);

        let gem = spawn_gem(
            &mut commands,
            translation,
            (*typ as u8).into(),
            &gltf_assets,
            &assets,
        );

        commands
            .spawn_bundle(PbrBundle {
                transform: Transform::from_translation(translation),
                mesh: assets.cube.clone_weak(),
                material: assets.transparent.clone_weak(),
                ..default()
            })
            .insert_bundle((
                GemSlot {
                    pos: *pos,
                    gem: Some(gem),
                },
                RayCastMesh::<RaycastSet>::default(),
            ));
    });
    commands.insert_resource(SelectedSlot(None));
}

fn gem_events(mut events: ResMut<BoardEvents>) {
    if let Ok(event) = events.pop() {
        match event {
            BoardEvent::Swapped(from, to) => info!("Swapped from {from} to {to}"),
            BoardEvent::FailedSwap(from, to) => info!("Failed to swap from {from} to {to}"),
            BoardEvent::Dropped(drops) => info!("Dropped {drops:?}"),
            BoardEvent::Popped(pop) => info!("Popped {pop}"),
            BoardEvent::Spawned(spawns) => info!("Spawned {spawns:?}"),
            BoardEvent::Matched(matches) => info!("Matched {:?}", matches.without_duplicates()),
        }
    }
}

fn spawn_gem(
    commands: &mut Commands,
    pos: Vec3,
    typ: GemType,
    gltf_assets: &Res<Assets<Gltf>>,
    assets: &Res<GemAssets>,
) -> Entity {
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
        })
        .id()
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

struct RaycastSet;

#[derive(Component)]
struct GemSlot {
    pos: UVec2,
    gem: Option<Entity>,
}

fn update_raycast_with_cursor(
    mut cursor: EventReader<CursorMoved>,
    mut query: Query<&mut RayCastSource<RaycastSet>>,
) {
    for mut pick_source in &mut query.iter_mut() {
        if let Some(cursor_latest) = cursor.iter().last() {
            pick_source.cast_method = RayCastMethod::Screenspace(cursor_latest.position);
        }
    }
}

fn select(
    mouse_buttons: Res<Input<MouseButton>>,
    mut selected: ResMut<SelectedSlot>,
    mut board_commands: ResMut<BoardCommands>,
    from: Query<&RayCastSource<RaycastSet>>,
    to: Query<&GemSlot>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for raycast_source in from.iter() {
        let (hit_entity, hit_slot) = match raycast_source
            .intersect_top()
            .and_then(|(hit, _)| to.get(hit).map(|hit_slot| (hit, hit_slot)).ok())
        {
            Some(val) => val,
            None => {
                **selected = None;
                continue;
            }
        };

        let previously_selected_slot =
            selected.and_then(|selected_slot| to.get(selected_slot).ok());

        if previously_selected_slot
            .and_then(|slot| slot.gem)
            .is_some_and(|previous_gem| hit_slot.gem.is_some_and(|hit_gem| hit_gem == previous_gem))
        {
            **selected = None;
            continue;
        }

        if let Some(previously_selected_slot) = previously_selected_slot {
            board_commands
                .push(BoardCommand::Swap(previously_selected_slot.pos, hit_slot.pos))
                .unwrap();
            **selected = None;
        } else {
            **selected = Some(hit_entity);
        }
    }
}

fn animate_selected(
    mut commands: Commands,
    selected: Res<SelectedSlot>,
    mut prev_selected: Local<Option<SelectedSlot>>,
    slots: Query<&GemSlot>,
    mut animators: Query<(&mut Transform, &mut Animator<Transform>)>,
) {
    if !selected.is_changed() {
        return;
    }

    // stop old animation, if any
    if let Some((mut transform, mut animator)) = (*prev_selected)
        .and_then(|prev_selected| *prev_selected)
        .and_then(|prev_selected| slots.get(prev_selected).ok())
        .and_then(|selected_gem| selected_gem.gem)
        .and_then(|selected_gem| animators.get_mut(selected_gem).ok())
    {
        animator.stop();
        transform.rotation = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, 0.0);
    }

    // animate new selection
    if let Some(selected_gem) = (**selected).and_then(|selected_slot| {
        slots
            .get(selected_slot)
            .expect("Selected slot entity is not a gem??")
            .gem
    }) {
        let seq = Tween::new(
            EaseFunction::SineInOut,
            TweeningType::PingPong,
            Duration::from_secs_f32(0.3),
            TransformRotateZLens {
                start: -0.5,
                end: 0.5,
            },
        );
        commands.entity(selected_gem).insert(Animator::new(seq));
        *prev_selected = Some(*selected);
    }
}

#[derive(Deref, DerefMut, Clone, Copy)]
struct SelectedSlot(Option<Entity>);
