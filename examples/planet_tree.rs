use bevy::{
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
    render::{options::WgpuOptions, render_resource::WgpuFeatures},
    utils::HashMap,
};
use bevy_config_cam::*;
use spatial_trees::{
    NodeKey,
    planet_tree::{
        *,
        Direction,
    }
};
use std::time::*;

#[derive(Debug, Clone, Hash, PartialEq, Eq, SystemLabel)]
enum ProgramStages {
    MaterialInit,
}

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
        .insert_resource(PlanetTree::new(0.1, 10.0, [0.0, 0.0, 0.0]))
        .add_plugin(WireframePlugin)
        .add_startup_system(setup_camera)
        .add_startup_system(setup_material.label(ProgramStages::MaterialInit))       
        .add_startup_system(setup_axis_boxes)
        .add_system(toggle_wireframe_system)
        .add_system(check_planet_tree)
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

fn setup_axis_boxes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let offset = 15.0;
    for dir in all_neighbor_directions::<3>() {
        let float_dirs = Vec3::from([dir[0] as f32, dir[1] as f32, dir[2] as f32]);
        let mut vec_colors = float_dirs.to_array();
        vec_colors.iter_mut().for_each(|v| {
            if *v < 0.0 {
                *v = 0.5
            } else if *v > 0.0 {
                *v = 1.0
            }
        });

        let color = Color::from(vec_colors);
        let pos = float_dirs * offset;

        let mut material = StandardMaterial::from(color);
        material.unlit = true;

        commands.spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
            material: materials.add(material),
            transform: Transform::from_translation(pos),
            ..Default::default()
        });
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

fn check_planet_tree(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    plane_material: Res<PlaneMaterial>,
    mut spawned_nodes: Local<SpawnedNodes>,
    mut planet_tree: ResMut<PlanetTree>,
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
        if last_pos.distance(player_transform.translation) < planet_tree.min_size {
            return;
        }

        let player_pos = player_transform.translation;

        let now = Instant::now();

        let qt_events = planet_tree.insert_and_update_neighbors(|node| {
            let node_pos = Vec3::from(node.world_position());
            let distance = node_pos.distance(player_pos);
            let threshold = 3.0 * node.size();
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

                    // eprintln!("Grow Event - new_children: {}", children.len());

                    if let Some(parent_entity) = spawned_nodes.0.get(&parent) {
                        commands.entity(*parent_entity).despawn();
                        spawned_nodes.0.remove(&parent);
                    }

                    for new_child in children {
                        let child_node = &planet_tree.nodes[new_child];
                        let child_id =
                            spawn_plane(&mut commands, &mut meshes, &plane_material, child_node);
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
                        let child_node = &planet_tree.nodes[retained];
                        spawn_plane(&mut commands, &mut meshes, &plane_material, child_node)
                    });
                }
                TreeEvent::NeighborSizesChanged(_) => {
                    neighbor_changed_events += 1;
                }
            }
        }

        println!(" PlanetTree Changed: Took {:?} us", duration.as_micros());
        println!("\t Grow Events    : {} | {}", grow_events.0, grow_events.1);
        println!(
            "\t Shrink Events  : {} | {}",
            shrink_events.0, shrink_events.1
        );
        println!("\t Neighbor Events: {}", neighbor_changed_events);

        *last_pos = player_transform.translation
    }
}

fn spawn_plane(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    plane_material: &PlaneMaterial,
    node: &PlanetTreeNode,
) -> Entity {
    let rotation = map_to_rotation(node.direction());
    let world_pos = node.world_position();
    let mut transform = Transform::from_rotation(rotation);
    transform.translation = Vec3::from(world_pos);

    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane { size: node.size() })),
            material: plane_material.material_handle.as_ref().unwrap().clone(),
            transform,
            ..Default::default()
        })
        .id()
}

fn map_to_rotation(dir: Direction) -> Quat {
    match dir {
        Direction::XPos => Quat::from_rotation_z(-90f32.to_radians()),
        Direction::XNeg => Quat::from_rotation_z(90f32.to_radians()),
        Direction::YPos => Quat::IDENTITY,
        Direction::YNeg => Quat::from_rotation_z(180f32.to_radians()),
        Direction::ZPos => Quat::from_rotation_x(90f32.to_radians()),
        Direction::ZNeg => Quat::from_rotation_x(-90f32.to_radians()),
        Direction::None => Quat::IDENTITY,
    }
}

#[derive(Component)]
struct NeighborBox;

fn check_neighbors(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    key: Res<Input<KeyCode>>,
    planet_tree: Res<PlanetTree>,
    current_neighbors: Query<Entity, With<NeighborBox>>,
) {
    if key.just_pressed(KeyCode::N) {
        for entity in current_neighbors.iter() {
            commands.entity(entity).despawn();
        }

        let leaf_nodes = planet_tree.iter_leaf_nodes().collect::<Vec<_>>();
        let random_node_index = fastrand::usize(..leaf_nodes.len());
        let (node_key, node) = leaf_nodes[random_node_index];

        //println!("\n * Find neighbors of {:?}", node);
        spawn_box_with_color(
            &mut commands,
            &mut meshes,
            &mut materials,
            node,
            Color::YELLOW,
        );

        for direction in &[[-1, 0], [1, 0], [0, -1], [0, 1]] {
            // println!("Trying to find neighbor in direction {:?}", direction);
            for neighbor in planet_tree.get_neighbors(node_key, *direction) {
                // eprintln!(
                //     "Neighbor in dir {:?} = {:?}",
                //     direction, &planet_tree.nodes[neighbor]
                // );
                spawn_box_with_color(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &planet_tree.nodes[neighbor],
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
    node: &PlanetTreeNode,
    color: Color,
) {
    let mut material = StandardMaterial::from(color);
    material.unlit = true;

    let pos = node.world_position();

    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube {
                size: node.size() / 2.0,
            })),
            material: materials.add(material),
            transform: Transform::from_xyz(pos[0], pos[1], pos[2]),
            ..Default::default()
        })
        .insert(NeighborBox);
}
