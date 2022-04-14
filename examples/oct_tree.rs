use bevy::{
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
    render::{options::WgpuOptions, render_resource::WgpuFeatures},
    utils::HashMap,
};
use bevy_config_cam::*;
use spatial_trees::{
    NodeKey,
    oct_tree::*
};
use std::time::*;

fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(ConfigCam)
        .insert_resource(WgpuOptions {
            features: WgpuFeatures::POLYGON_MODE_LINE,
            ..Default::default()
        })
        .insert_resource(PlaneMaterial {
            material_handle: None,
        })
        .insert_resource(OctTree::new(1., 100.0, [0.0, 0.0, 0.0]))
        .add_plugin(WireframePlugin)
        .add_startup_system(setup_camera)
        .add_startup_system(setup_material)
        .add_system(toggle_wireframe_system)
        .add_system(check_oct_tree)
        .add_system(check_neighbors)
        .run();
}

fn setup_camera(
    mut cam_state: ResMut<State<CameraState>>,
    mut commands: Commands,
    player_query: Query<Entity, With<PlayerMove>>,
) {
    cam_state.set(CameraState::Free).unwrap();
    if let Some(player_entity) = player_query.get_single().ok() {
        commands.entity(player_entity).despawn_recursive()
    }
}

struct PlaneMaterial {
    material_handle: Option<Handle<StandardMaterial>>,
}

fn setup_material(
    //mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut plane_material: ResMut<PlaneMaterial>,
    asset_server: ResMut<AssetServer>,
) {
    let handle = asset_server.load("debug.png");
    let mut material = StandardMaterial::from(handle);
    material.unlit = true;

    plane_material.material_handle = Some(materials.add(material));
}

fn toggle_wireframe_system(
    key: Res<Input<KeyCode>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if key.just_pressed(KeyCode::F) {
        wireframe_config.global = !wireframe_config.global;
    }
}

#[derive(Default)]
struct SpawnedNodes(HashMap<NodeKey, Entity>);

fn check_oct_tree(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned_nodes: Local<SpawnedNodes>,
    mut oct_tree: ResMut<OctTree>,
    mut last_pos: Local<Vec3>,
    player_query: Query<&Transform, With<Camera>>,
    key: Res<Input<KeyCode>>,
) {
    if !key.just_pressed(KeyCode::G) {
        return;
    }

    let player_tr = if player_query.iter().count() == 1 {
        player_query.get_single().ok()
    } else {
        player_query.iter().nth(1)
    };

    if let Some(player_transform) = player_tr {
        if last_pos.distance(player_transform.translation) < oct_tree.min_size {
            return;
        }

        let player_pos = player_transform.translation;

        let now = Instant::now();

        let qt_events = oct_tree.insert_and_update_neighbors(|node| {
            let node_pos = Vec3::from(node.pos);
            let distance = node_pos.distance(player_pos);
            let threshold = 3.0 * node.size;
            let allowed = distance < threshold;
            //eprintln!("Player {:?}, Node {:?}, {} < {} = {}.. Has children? {}", player_pos, node_pos, distance, threshold, allowed, node.has_children());
            allowed
        });

        let duration = now.elapsed();

        let mut grow_events = (0, 0);
        let mut shrink_events = (0, 0);
        let mut neighbor_changed_events = 0;

        for event in qt_events {
            match event {
                TreeEvent::Grown { parent, children } => {
                    grow_events.0 += 1;
                    grow_events.1 += children.len();

                    if let Some(parent_entity) = spawned_nodes.0.get(&parent) {
                        // eprintln!("despawning {:?}", parent);
                        commands.entity(*parent_entity).despawn();
                        spawned_nodes.0.remove(&parent);
                    }

                    for new_child in children {
                        let child_node = &oct_tree.nodes[new_child];
                        let child_id =
                            spawn_oct_box(&mut commands, &mut meshes, &mut materials, child_node);
                        spawned_nodes.0.insert(new_child, child_id);
                    }
                }
                TreeEvent::Shrunk { retained, removed } => {
                    shrink_events.0 += 1;
                    shrink_events.1 += removed.len();

                    for removed_node in removed {
                        if let Some(node_entity) = spawned_nodes.0.get(&removed_node) {
                            commands.entity(*node_entity).despawn();
                            spawned_nodes.0.remove(&removed_node);
                        }
                    }

                    spawned_nodes.0.entry(retained).or_insert_with(|| {
                        let child_node = &oct_tree.nodes[retained];
                        spawn_oct_box(&mut commands, &mut meshes, &mut materials, child_node)
                    });
                }
                TreeEvent::NeighborSizesChanged(_) => {
                    neighbor_changed_events += 1;
                }
            }
        }

        println!(" OctTree Changed: Took {:?} us", duration.as_micros());
        println!("\t Grow Events    : {} | {}", grow_events.0, grow_events.1);
        println!(
            "\t Shrink Events  : {} | {}",
            shrink_events.0, shrink_events.1
        );
        println!("\t Neighbor Events: {}", neighbor_changed_events);

        *last_pos = player_transform.translation
    }
}

#[derive(Component)]
struct NeighborBox;

fn check_neighbors(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    key: Res<Input<KeyCode>>,
    oct_tree: Res<OctTree>,
    mut last_node: Local<Option<usize>>,
    current_neighbors: Query<Entity, With<NeighborBox>>,
) {
    if key.just_pressed(KeyCode::N) {
        for entity in current_neighbors.iter() {
            commands.entity(entity).despawn();
        }

        let leaf_nodes = oct_tree.iter_leaf_nodes().collect::<Vec<_>>();
        let leaf_index = if let Some(last_node_index) = &mut *last_node {
            if *last_node_index < leaf_nodes.len() {
                *last_node_index += 1;
                *last_node_index
            } else {
                *last_node = Some(0);
                0
            }
        } else {
            *last_node = Some(0);
            0
        };

        let (node_key, node) = leaf_nodes[leaf_index];

        // println!("\n * Find neighbors of {:?}", node);
        spawn_box_with_color(
            &mut commands,
            &mut meshes,
            &mut materials,
            node,
            Color::YELLOW,
        );

        for direction in all_neighbor_directions::<3>() {
            // println!("Trying to find neighbor in direction {:?}", direction);
            for neighbor in oct_tree.get_neighbors(node_key, direction) {
                // eprintln!(
                //     "Neighbor in dir {:?} = {:?}",
                //     direction, &oct_tree.nodes[neighbor]
                // );
                spawn_box_with_color(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &oct_tree.nodes[neighbor],
                    Color::BLUE,
                )
            }
        }
    }
}

fn spawn_box_with_color(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node: &OctTreeNode,
    color: Color,
) {
    let mut material = StandardMaterial::from(color);
    material.unlit = true;

    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube {
                size: node.size / 2.0,
            })),
            material: materials.add(material),
            transform: Transform::from_xyz(node.pos[0], node.pos[1], node.pos[2]),
            ..Default::default()
        })
        .insert(NeighborBox);
}

fn spawn_oct_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node: &OctTreeNode,
) -> Entity {
    let color = Color::rgba_u8(
        fastrand::u8(0..u8::MAX),
        fastrand::u8(0..u8::MAX),
        fastrand::u8(0..u8::MAX),
        32,
    );
    let mut material = StandardMaterial::from(color);
    material.unlit = true;
    material.alpha_mode = AlphaMode::Blend;
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: node.size })),
            material: materials.add(material),
            transform: Transform::from_xyz(node.pos[0], node.pos[1], node.pos[2]),
            ..Default::default()
        })
        .id()
}
