#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![feature(is_some_with)]

use std::time::Duration;

use assets::{load_assets, GemAssets};
use bevy::{app::AppExit, gltf::Gltf, prelude::*, utils::HashMap};
use bevy_egui::{
    egui::{self, FontId, RichText},
    EguiContext, EguiPlugin,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_match3::{prelude::*, Match3Config};
use bevy_mod_raycast::{DefaultRaycastingPlugin, RayCastMesh, RayCastMethod, RayCastSource};
use bevy_tweening::{
    lens::*, Animator, EaseFunction, EaseMethod, Tween, TweeningPlugin, TweeningType,
};
use heron::PhysicsPlugin;
use strum::{Display, EnumIter, IntoEnumIterator};

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
        // .add_plugin(WorldInspectorPlugin::default())
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
        .add_system_set(
            SystemSet::on_enter(GameState::Game)
                .with_system(spawn_board)
                .with_system(setup_resources),
        )
        .add_system_set(
            SystemSet::on_update(GameState::Game)
                .with_system(gem_events)
                .with_system(update_raycast_with_cursor)
                .with_system(select.after(gem_events))
                .with_system(animate_selected)
                .with_system(left_sidebar)
                .with_system(right_sidebar),
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
                ui.heading(RichText::new("UNTITLED MATCH 3 RPG").font(FontId::monospace(100.0)));
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
    board.iter().for_each(|(pos, typ)| {
        let translation = gem_pos_from(pos);

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

fn gem_pos_from(pos: &UVec2) -> Vec3 {
    let size = 0.2;
    let top = (size * 4.0) - (size / 2.0);
    let left = -(size * 4.0) + (size / 2.0);
    Vec3::new(left + pos.x as f32 * size, top - pos.y as f32 * size, 0.0)
}

fn gem_events(
    mut commands: Commands,
    mut events: ResMut<BoardEvents>,
    mut board_commands: ResMut<BoardCommands>,
    gltf_assets: Res<Assets<Gltf>>,
    assets: Res<GemAssets>,
    gems: Query<(&Transform, Option<&Animator<Transform>>, Entity, &GemType)>,
    mut slots: Query<(&Transform, &mut GemSlot)>,
    mut resources: Query<&mut Resources, With<Player>>,
) {
    // Only read new events if we're done moving gems around
    for (animator, entity) in gems
        .iter()
        .filter_map(|(_, animator, entity, _)| animator.map(|animator| (animator, entity)))
    {
        if animator.progress() != 1.0 {
            return;
        } else {
            commands.entity(entity).remove::<Animator<Transform>>();
        }
    }

    if let Ok(event) = events.pop() {
        match event {
            BoardEvent::Swapped(from, to) => {
                info!("Swapped from {from} to {to}");
                let from_gem = get_gem_from_pos(from, &slots);
                let to_gem = get_gem_from_pos(to, &slots);

                swap_gems_in_slots(
                    &GemSlot {
                        pos: from,
                        gem: Some(from_gem),
                    },
                    &GemSlot {
                        pos: to,
                        gem: Some(to_gem),
                    },
                    &mut slots,
                );

                let from_transform = gems.get_component::<Transform>(from_gem).unwrap();
                let to_transform = gems.get_component::<Transform>(to_gem).unwrap();
                commands.entity(from_gem).insert(Animator::new(Tween::new(
                    EaseFunction::QuadraticInOut,
                    TweeningType::Once,
                    Duration::from_secs_f32(0.5),
                    TransformPositionLens {
                        start: from_transform.translation,
                        end: to_transform.translation,
                    },
                )));
                commands.entity(to_gem).insert(Animator::new(Tween::new(
                    EaseFunction::QuadraticInOut,
                    TweeningType::Once,
                    Duration::from_secs_f32(0.5),
                    TransformPositionLens {
                        start: to_transform.translation,
                        end: from_transform.translation,
                    },
                )));
            }
            BoardEvent::FailedSwap(from, to) => {
                info!("Failed to swap from {from} to {to}");

                let from_gem = get_gem_from_pos(from, &slots);
                let to_gem = get_gem_from_pos(to, &slots);

                let from_transform = gems.get_component::<Transform>(from_gem).unwrap();
                let to_transform = gems.get_component::<Transform>(to_gem).unwrap();

                commands.entity(from_gem).insert(Animator::new(
                    Tween::new(
                        EaseFunction::QuadraticInOut,
                        TweeningType::Once,
                        Duration::from_secs_f32(0.25),
                        TransformPositionLens {
                            start: from_transform.translation,
                            end: to_transform.translation,
                        },
                    )
                    .then(Tween::new(
                        EaseFunction::QuadraticInOut,
                        TweeningType::Once,
                        Duration::from_secs_f32(0.25),
                        TransformPositionLens {
                            start: to_transform.translation,
                            end: from_transform.translation,
                        },
                    )),
                ));
                commands.entity(to_gem).insert(Animator::new(
                    Tween::new(
                        EaseFunction::QuadraticInOut,
                        TweeningType::Once,
                        Duration::from_secs_f32(0.25),
                        TransformPositionLens {
                            start: to_transform.translation,
                            end: from_transform.translation,
                        },
                    )
                    .then(Tween::new(
                        EaseFunction::QuadraticInOut,
                        TweeningType::Once,
                        Duration::from_secs_f32(0.25),
                        TransformPositionLens {
                            start: from_transform.translation,
                            end: to_transform.translation,
                        },
                    )),
                ));
            }
            BoardEvent::Dropped(drops) => {
                info!("Dropped {drops:?}");
                for Drop { from, to } in drops.iter().copied() {
                    let from_gem = get_gem_from_pos(from, &slots);
                    let (to_transform, to_slot) = get_slot_from_pos(to, &slots);

                    swap_gems_in_slots(
                        &GemSlot {
                            pos: from,
                            gem: Some(from_gem),
                        },
                        &to_slot,
                        &mut slots,
                    );

                    let from_transform = gems.get_component::<Transform>(from_gem).unwrap();
                    commands.entity(from_gem).insert(Animator::new(Tween::new(
                        EaseFunction::CubicIn,
                        TweeningType::Once,
                        Duration::from_secs_f32(0.25),
                        TransformPositionLens {
                            start: from_transform.translation,
                            end: to_transform.translation,
                        },
                    )));
                }
            }
            BoardEvent::Popped(pop) => {
                info!("Popped {pop}");
                let mut slot = slots.iter_mut().find(|slot| slot.1.pos == pop).unwrap().1;
                let gem = slot.gem.unwrap();
                let typ = gems.get_component::<GemType>(gem).unwrap();
                resources.single_mut().add(typ);
                commands.entity(gem).despawn_recursive();
                slot.gem = None;
            }
            BoardEvent::Spawned(spawns) => {
                info!("Spawned {spawns:?}");
                for (pos, typ) in spawns.iter().copied() {
                    let typ = GemType::from(typ as u8);
                    let (transform, mut slot) =
                        slots.iter_mut().find(|(_, slot)| slot.pos == pos).unwrap();
                    let mut start_pos = transform.translation;
                    // offset starting position by about a board length so they drop in from off screen
                    start_pos.y += 0.2 * 8.0;
                    let gem = spawn_gem(&mut commands, start_pos, typ, &gltf_assets, &assets);
                    commands.entity(gem).insert(Animator::new(Tween::new(
                        EaseMethod::Linear,
                        TweeningType::Once,
                        Duration::from_secs_f32(0.25),
                        TransformPositionLens {
                            start: start_pos,
                            end: transform.translation,
                        },
                    )));

                    slot.gem = Some(gem);
                }
            }
            BoardEvent::Matched(matches) => {
                info!("Matched {:?}", matches.without_duplicates());
                board_commands
                    .push(BoardCommand::Pop(
                        matches.without_duplicates().iter().copied().collect(),
                    ))
                    .unwrap();
            }
        }
    }
}

fn swap_gems_in_slots(
    slot1: &GemSlot,
    slot2: &GemSlot,
    slots: &mut Query<(&Transform, &mut GemSlot)>,
) {
    slots.for_each_mut(|(_, mut slot)| {
        if slot.pos == slot1.pos {
            slot.gem = slot2.gem;
        } else if slot.pos == slot2.pos {
            slot.gem = slot1.gem;
        }
    });
}

fn get_gem_from_pos(pos: UVec2, slots: &Query<(&Transform, &mut GemSlot)>) -> Entity {
    get_slot_from_pos(pos, slots).1.gem.unwrap()
}

fn get_slot_from_pos(
    pos: UVec2,
    slots: &Query<(&Transform, &mut GemSlot)>,
) -> (Transform, GemSlot) {
    slots
        .iter()
        .find(|(_, slot)| slot.pos == pos)
        .map(|(t, s)| (*t, *s))
        .unwrap()
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
#[derive(Component, Clone, Copy, EnumIter, Display, Eq, Hash, PartialEq)]
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

#[derive(Component, Copy, Clone)]
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
    mut commands: Commands,
    mouse_buttons: Res<Input<MouseButton>>,
    mut selected: ResMut<SelectedSlot>,
    mut board_commands: ResMut<BoardCommands>,
    from: Query<&RayCastSource<RaycastSet>>,
    to: Query<&GemSlot>,
    gems: Query<(&Animator<Transform>, Entity), With<GemType>>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    // Only allow selection if no gems (other than currently selected gem) are moving
    for (animator, entity) in gems.iter() {
        // if a gem is selected and is moving, check next gem
        if selected.is_some_and(|selected| {
            to.get(*selected).is_ok_and(|selected_slot| {
                selected_slot
                    .gem
                    .is_some_and(|selected_gem| *selected_gem == entity)
            })
        }) {
            continue;
        }
        // if gem is moving, return
        else if animator.progress() != 1.0 {
            return;
        }
        // gem has finished moving, remove animator
        else {
            commands.entity(entity).remove::<Animator<Transform>>();
        }
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
            if previously_selected_slot
                .pos
                .cardinally_adjacent(&hit_slot.pos)
            {
                board_commands
                    .push(BoardCommand::Swap(
                        previously_selected_slot.pos,
                        hit_slot.pos,
                    ))
                    .unwrap();
            }
            **selected = None;
        } else {
            **selected = Some(hit_entity);
        }
    }
}

trait BoardPosition {
    fn left(&self) -> Self;
    fn right(&self) -> Self;
    fn up(&self) -> Self;
    fn down(&self) -> Self;
    fn cardinally_adjacent(&self, other: &Self) -> bool;
}

impl BoardPosition for UVec2 {
    fn left(&self) -> Self {
        Self::new(self.x.saturating_sub(1), self.y)
    }

    fn right(&self) -> Self {
        Self::new(self.x.saturating_add(1), self.y)
    }

    fn up(&self) -> Self {
        Self::new(self.x, self.y.saturating_sub(1))
    }

    fn down(&self) -> Self {
        Self::new(self.x, self.y.saturating_add(1))
    }

    fn cardinally_adjacent(&self, other: &Self) -> bool {
        self == &other.left()
            || self == &other.right()
            || self == &other.up()
            || self == &other.down()
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
    if let Some((mut transform, mut animator, entity)) = (*prev_selected)
        .as_deref()
        .copied()
        .flatten()
        .and_then(|prev_selected| slots.get(prev_selected).ok())
        .and_then(|selected_gem| selected_gem.gem)
        .and_then(|selected_gem| {
            animators
                .get_mut(selected_gem)
                .ok()
                .map(|gem| (gem.0, gem.1, selected_gem))
        })
    {
        animator.stop();
        transform.rotation = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, 0.0);
        commands.entity(entity).remove::<Animator<Transform>>();
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

fn left_sidebar(
    mut egui_ctx: ResMut<EguiContext>,
    windows: Res<Windows>,
    resources: Query<&Resources, With<Player>>,
) {
    let window = windows.primary();
    let resources = resources.single();
    egui::SidePanel::left("Player panel")
        .resizable(false)
        .show(egui_ctx.ctx_mut(), |ui| {
            ui.set_width(window.width() / 4.0);
            ui.with_layout(
                egui::Layout::default().with_cross_align(egui::Align::Center),
                |ui| {
                    ui.heading(RichText::new("Player").font(FontId::monospace(50.0)));
                    ui.separator();
                    for typ in GemType::iter() {
                        ui.label(format!(
                            "{typ}: {}",
                            resources.mana.get(&typ).copied().unwrap_or_default()
                        ));
                    }
                    ui.separator();
                    if ui
                        .add_enabled(false, egui::Button::new(RichText::new("Bonk: 3equipment")))
                        .clicked()
                    {}
                    if ui
                        .add_enabled(false, egui::Button::new(RichText::new("Heal: 3amethyst")))
                        .clicked()
                    {}
                },
            );
        });
}

fn right_sidebar(mut egui_ctx: ResMut<EguiContext>, windows: Res<Windows>) {
    let window = windows.primary();
    egui::SidePanel::right("Opponent panel")
        .resizable(false)
        .show(egui_ctx.ctx_mut(), |ui| {
            ui.set_width(window.width() / 4.0);
            ui.with_layout(
                egui::Layout::default().with_cross_align(egui::Align::Center),
                |ui| {
                    ui.heading(RichText::new("Opponent").font(FontId::monospace(50.0)));
                    ui.label("Some stuff");
                },
            );
        });
}

#[derive(Component, Default)]
struct Resources {
    mana: HashMap<GemType, u32>,
}

impl Resources {
    fn add(&mut self, typ: &GemType) {
        self.mana
            .insert(*typ, self.mana.get(typ).copied().unwrap_or_default() + 1);
    }
}

#[derive(Component)]
struct Player;

fn setup_resources(mut commands: Commands) {
    // Player resources
    commands.spawn_bundle((Player, Resources::default()));
    // Opponent resources
    commands.spawn_bundle((Resources::default(),));
}
