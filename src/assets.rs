use bevy::{gltf::Gltf, prelude::*, utils::HashMap};
use strum::{Display, EnumIter, IntoEnumIterator};

use crate::GemType;

#[derive(Display, EnumIter, Eq, Hash, PartialEq, Clone, Copy)]
pub enum GemShape {
    Asscher,
    Baguette,
    Marquise,
    Pear,
    Round,
    Trillion,
    Skull,
    Equipment,
}

impl GemShape {
    pub fn mesh_path(&self) -> String {
        format!("{self}.glb")
    }

    pub fn shattered_mesh_path(&self) -> String {
        format!("{self}_shattered.glb")
    }
}

impl From<GemType> for GemShape {
    fn from(typ: GemType) -> Self {
        match typ {
            GemType::Ruby => GemShape::Asscher,
            GemType::Emerald => GemShape::Baguette,
            GemType::Sapphire => GemShape::Marquise,
            GemType::Topaz => GemShape::Pear,
            GemType::Diamond => GemShape::Round,
            GemType::Amethyst => GemShape::Trillion,
            GemType::Skull => GemShape::Skull,
            GemType::Equipment => GemShape::Equipment,
        }
    }
}

#[derive(Default)]
pub struct GemAssets {
    pub meshes: HashMap<GemShape, Handle<Gltf>>,
    pub shatter_meshes: HashMap<GemShape, Handle<Gltf>>,
    pub materials: Vec<Handle<StandardMaterial>>,
}

pub fn load_assets(
    mut commands: Commands,
    ass: Res<AssetServer>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut assets = GemAssets::default();
    for shape in GemShape::iter() {
        assets.meshes.insert(shape, ass.load(&shape.mesh_path()));
        assets
            .shatter_meshes
            .insert(shape, ass.load(&shape.shattered_mesh_path()));
    }

    for color in [
        Color::RED,
        Color::GREEN,
        Color::BLUE,
        Color::YELLOW,
        Color::WHITE,
        Color::PURPLE,
        Color::ANTIQUE_WHITE,
        Color::GRAY,
    ] {
        assets.materials.push(mats.add(color.into()));
    }

    commands.insert_resource(assets);
}
