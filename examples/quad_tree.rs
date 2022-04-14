use bevy::{
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
    render::{options::WgpuOptions, render_resource::WgpuFeatures},
    utils::HashMap,
};
use bevy_config_cam::*;
use spatial_trees::{
    NodeKey,
    quad_tree::*
};

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
        .insert_resource(QuadTree::new(1., 10.0, [0.0, 0.0]))
        .add_plugin(WireframePlugin)
        .add_startup_system(setup_camera)
        .add_startup_system(setup_material)
        .add_system(toggle_wireframe_system)
        .add_system(check_quad_tree)
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

fn check_quad_tree(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    plane_material: Res<PlaneMaterial>,
    mut spawned_nodes: Local<SpawnedNodes>,
    mut quad_tree: ResMut<QuadTree>,
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
        if last_pos.distance(player_transform.translation) < quad_tree.min_size {
            return;
        }

        let player_pos = Vec2::new(
            player_transform.translation.x,
            player_transform.translation.z,
        );

        let qt_events = quad_tree.insert_and_update_neighbors(|node| {
            let node_pos = Vec2::from(node.pos);
            let distance = node_pos.distance(player_pos);
            let threshold = 3.0 * node.size;
            distance < threshold
        });

        for event in qt_events {
            match event {
                TreeEvent::Grown { parent, children } => {
                    if let Some(parent_entity) = spawned_nodes.0.get(&parent) {
                        commands.entity(*parent_entity).despawn();
                        spawned_nodes.0.remove(&parent);
                    }

                    for new_child in children {
                        let child_node = &quad_tree.nodes[new_child];
                        let child_id = commands
                            .spawn_bundle(PbrBundle {
                                mesh: meshes.add(Mesh::from(shape::Plane {
                                    size: child_node.size,
                                })),
                                material: plane_material.material_handle.as_ref().unwrap().clone(),
                                transform: Transform::from_xyz(
                                    child_node.pos[0],
                                    0.0,
                                    child_node.pos[1],
                                ),
                                ..Default::default()
                            })
                            .id();
                        spawned_nodes.0.insert(new_child, child_id);
                    }
                }
                TreeEvent::Shrunk { retained, removed } => {
                    for removed_node in removed {
                        if let Some(node_entity) = spawned_nodes.0.get(&removed_node) {
                            commands.entity(*node_entity).despawn();
                            spawned_nodes.0.remove(&removed_node);
                        }
                    }

                    spawned_nodes.0.entry(retained).or_insert_with(|| {
                        let child_node = &quad_tree.nodes[retained];
                        commands
                            .spawn_bundle(PbrBundle {
                                mesh: meshes.add(Mesh::from(shape::Plane {
                                    size: child_node.size,
                                })),
                                material: plane_material.material_handle.as_ref().unwrap().clone(),
                                transform: Transform::from_xyz(
                                    child_node.pos[0],
                                    0.0,
                                    child_node.pos[1],
                                ),
                                ..Default::default()
                            })
                            .id()
                    });
                }
                _ => {}
            }
        }

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
    quad_tree: Res<QuadTree>,
    current_neighbors: Query<Entity, With<NeighborBox>>,
) {
    if key.just_pressed(KeyCode::N) {
        for entity in current_neighbors.iter() {
            commands.entity(entity).despawn();
        }

        let leaf_nodes = quad_tree.iter_leaf_nodes().collect::<Vec<_>>();
        let random_node_index = fastrand::usize(..leaf_nodes.len());
        let (node_key, node) = leaf_nodes[random_node_index];

        // println!("\n * Find neighbors of {:?}", node);
        spawn_box_with_color(
            &mut commands,
            &mut meshes,
            &mut materials,
            node,
            Color::YELLOW,
        );

        for direction in &[[-1, 0], [1, 0], [0, -1], [0, 1]] {
            // println!("Trying to find neighbor in direction {:?}", direction);
            for neighbor in quad_tree.get_neighbors(node_key, *direction) {
                // eprintln!(
                //     "Neighbor in dir {:?} = {:?}",
                //     direction, &quad_tree.nodes[neighbor]
                // );
                spawn_box_with_color(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &quad_tree.nodes[neighbor],
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
    node: &QuadTreeNode,
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
            transform: Transform::from_xyz(node.pos[0], 0.0, node.pos[1]),
            ..Default::default()
        })
        .insert(NeighborBox);
}
