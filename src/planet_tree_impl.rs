use ahash::AHashMap as HashMap;

use crate::{
    node_traits::*,
    tree_traits::*,
    NodeKey,
    planet_tree_node::*, 
};
use slotmap::SlotMap;

pub struct PlanetTree {
    pub nodes: SlotMap<NodeKey, PlanetTreeNode>,
    pub min_size: f32,
    roots: [NodeKey; 6],
}

impl PlanetTree {
    pub fn new(min_size: f32, size: f32, pos: [f32; 3]) -> Self {
        let mut nodes = SlotMap::default();
        let mut node_keys = vec![];
        for direction in &[
            [-1, 0, 0],
            [1, 0, 0],
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
        ] {
            let mut world_pos = pos;
            world_pos
                .iter_mut()
                .zip(direction.iter())
                .for_each(|(pos, dir)| *pos += (*dir as f32) * size / 2.0);
            let dir = *direction;
            let local_pos = map_from_dir_and_world_pos(dir.into(), world_pos);
            let root_node = PlanetTreeNode::new(size, local_pos, world_pos, dir.into());
            node_keys.push(nodes.insert(root_node))
        }

        Self {
            nodes,
            min_size,
            roots: node_keys.try_into().unwrap(),
        }
    }

    pub fn iter_leaf_nodes(&self) -> impl Iterator<Item = (NodeKey, &PlanetTreeNode)> {
        self.nodes.iter().filter(|(_, node)| !node.has_children())
    }
}

impl NodeStorage for PlanetTree {
    type NodeType = PlanetTreeNode;
    type NodeKeyType = NodeKey;

    fn get_node(&self, node_key: Self::NodeKeyType) -> Option<&Self::NodeType> {
        self.nodes.get(node_key)
    }

    fn get_mut_node(&mut self, node_key: Self::NodeKeyType) -> Option<&mut Self::NodeType> {
        self.nodes.get_mut(node_key)
    }

    fn get_node_unchecked(&self, node_key: Self::NodeKeyType) -> &Self::NodeType {
        &self.nodes[node_key]
    }

    fn get_mut_node_unchecked(&mut self, node_key: Self::NodeKeyType) -> &mut Self::NodeType {
        &mut self.nodes[node_key]
    }

    fn insert_node(&mut self, node: Self::NodeType) -> Self::NodeKeyType {
        self.nodes.insert(node)
    }
    fn remove_node(&mut self, node_key: Self::NodeKeyType) -> Option<Self::NodeType> {
        self.nodes.remove(node_key)
    }
}

impl TreeBehaviour<2> for PlanetTree {
    fn min_size(&self) -> f32 {
        self.min_size
    }

    fn root_items(&self) -> Vec<NodeKey> {
        self.roots.to_vec()
    }

    fn create_children(&mut self, parent_key: NodeKey) -> Vec<NodeKey> {
        let (parent_size, parent_pos, parent_direction, parent_world_position) = {
            let parent = self.get_node_unchecked(parent_key);
            (
                parent.size(),
                parent.pos(),
                parent.direction(),
                parent.world_position(),
            )
        };

        let new_size = parent_size / 2.0;
        let quart_size = parent_size / 4.0;

        let mut new_child_indexes = vec![];
        let num_children = 2usize.pow(2);

        for child_index in 0..num_children {
            let pos = child_position::<2>(child_index);
            let mut child_pos = parent_pos;
            child_pos.iter_mut().zip(pos.iter()).for_each(|(out, p)| {
                let v = *out + *p as f32 * quart_size;
                *out = v;
            });

            let mut child = PlanetTreeNode::new(
                new_size,
                child_pos,
                map_from_dir_and_local_pos(parent_direction, child_pos, parent_world_position),
                parent_direction,
            );

            child.set_parent(parent_key);
            new_child_indexes.push(self.insert_node(child));
        }
        self.get_mut_node_unchecked(parent_key)
            .set_child_keys(new_child_indexes.as_slice());
        new_child_indexes
    }

    fn grow_event(
        &self,
        events: &mut Vec<TreeEvent>,
        pos: [f32; 2],
        parent_key: NodeKey,
        new_children: &[NodeKey],
    ) {
        let parent_direction = self.get_node_unchecked(parent_key).direction();
        for event in events.iter_mut().rev() {
            if let TreeEvent::Grown { parent, children } = event {
                let current_parrent_node = self.get_node_unchecked(*parent);
                if current_parrent_node.direction() == parent_direction
                    && current_parrent_node.contains_point(pos)
                {
                    children.retain(|node_key| !self.get_node_unchecked(*node_key).has_children());
                    children.extend(new_children.iter());
                    return;
                }
            }
        }
        events.push(TreeEvent::Grown {
            parent: parent_key,
            children: Vec::from_iter(new_children.iter().copied()),
        });
    }
}

impl TreeNeighbourBehaviour<2> for PlanetTree {
    fn find_shared_parent(
        &self,
        mut node_key: NodeKey,
        direction: [i32; 2],
    ) -> Option<(NodeKey, Vec<[i32; 2]>)> {
        let mut working_direction = direction;
        let mut neighbor_descents = vec![];

        //Find the shared parent between the node and the potentioal node in the neighbor direction.
        while let Some(parent) = self.get_node_unchecked(node_key).get_parent() {
            if working_direction.iter().all(|v| *v == 0) {
                break;
            }

            let node_descent = self
                .get_node_unchecked(parent)
                .child_position_from_key(node_key)
                .unwrap();

            let mut neighbor_descent = [0; 2];
            neighbor_descent
                .iter_mut()
                .zip(node_descent.iter().zip(working_direction.iter()))
                .for_each(|(out, (nd, dir))| *out = *nd * (1 - 2 * dir.abs()));

            neighbor_descents.push(neighbor_descent);

            working_direction
                .iter_mut()
                .zip(node_descent.iter())
                .for_each(|(wd, nd)| *wd = (*nd + *wd) / 2);

            node_key = parent;
        }

        // if direction is not [0, 0] then there is no shared parent inside this QuadTree. Find neighboring face of the PlanetTree
        if !working_direction.iter().all(|v| *v == 0) {
            let node = self.get_node_unchecked(node_key);
            let (dir, neighbor_transform) = map_to_neighbor(node.direction(), working_direction);
            //eprint!("{:?} : {:?}", neighbor_transform, neighbor_descents);
            match neighbor_transform {
                NeighborTransform::Mirror { axis } => neighbor_descents
                    .iter_mut()
                    .for_each(|descent| *descent = mirror(axis, *descent)),
                NeighborTransform::Rotate { clockwise } => neighbor_descents
                    .iter_mut()
                    .for_each(|descent| *descent = simple_rotate(clockwise, *descent)),
                NeighborTransform::RotateMirror { clockwise, axis } => {
                    neighbor_descents.iter_mut().for_each(|descent| {
                        *descent = simple_rotate(clockwise, *descent);
                        *descent = mirror(axis, *descent)
                    })
                }
                _ => {}
            };
            //eprintln!(" -> {:?}", neighbor_descents);
            
            node_key = self.roots[dir as usize];
        }

        Some((node_key, neighbor_descents))
    }

    fn update_neighbor_sizes(
        &mut self,
        node_key: NodeKey,
        visited_nodes: &mut HashMap<NodeKey, NeighborSizeEvent>,
    ) {
        let mut neighbor_sizes = vec![];
        let (node_size, node_dir) = {
            let node = self.get_node_unchecked(node_key);
            (node.size(), node.direction())
        };

        for direction in all_neighbor_directions::<2>() {
            let mut opposite_dir = direction;
            opposite_dir.iter_mut().for_each(|e| *e *= -1);
                                    
            for neighbour_key in self.get_neighbors(node_key, direction) {
                let (neighbour_size, neighbour_dir) = {
                    let node = self.get_node_unchecked(neighbour_key);
                    (node.size(), node.direction())
                };

                let neighbour_opposite_dir = map_from_dir_to_dir(node_dir, neighbour_dir, opposite_dir);                                
                if self.update_neighbor_size(node_key, neighbour_key, node_size, neighbour_opposite_dir)
                    == NeighborSizeEvent::ChangedSize
                {
                    visited_nodes
                        .entry(neighbour_key)
                        .or_insert(NeighborSizeEvent::ChangedSize);
                }

                if neighbour_size < node_size {
                    neighbor_sizes.push((direction, node_size, vec![0.0; 1]));
                } else {
                    neighbor_sizes.push((direction, neighbour_size, self.get_neighbor_offsets(node_key, neighbour_key, direction)));
                }
            }
        }

        let child_node = self.get_mut_node_unchecked(node_key);                 
        for (dir, size, offsets) in neighbor_sizes.iter() {
            let index = neighbor_index(*dir).unwrap();
            child_node.neighbor_sizes_mut()[index] = *size;
            let num_offsets = offsets.len();
            for (i, offset) in offsets.iter().enumerate() {
                let offset_index = index*num_offsets + i;
                child_node.neighbor_offsets_mut()[offset_index] = *offset;
            }            
        }
        visited_nodes.insert(node_key, NeighborSizeEvent::New);
    }

    fn get_neighbor_offsets(&self, node_key: NodeKey, neighbour_key: NodeKey, direction: [i32; 2]) -> Vec<f32> {
        let node = self.get_node_unchecked(node_key);
        let node_pos = node.pos();
        let neighbour = self.get_node_unchecked(neighbour_key);
        let neighbour_pos = map_from_dir_to_dir_float(neighbour.direction(), node.direction(), neighbour.pos());       
        
        direction.iter().zip(node_pos.iter()).zip(neighbour_pos.iter()).filter_map(|((dir, pos), neigh_pos)| {
            if *dir == 0 {
                Some(pos - neigh_pos)
            } else {
                None
            }
        }).collect()        
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    XNeg = 0,
    XPos = 1,
    YNeg = 2,
    YPos = 3,
    ZNeg = 4,
    ZPos = 5,
    None = 6,
}
impl From<usize> for Direction {
    fn from(v: usize) -> Self {
        match v {
            0 => Direction::XNeg,
            1 => Direction::XPos,
            2 => Direction::YNeg,
            3 => Direction::YPos,
            4 => Direction::ZNeg,
            5 => Direction::ZPos,
            _ => Direction::None, 
        }
    }
}
impl From<Direction> for [f32; 3] {
    fn from(v: Direction) -> Self {
        match v {
            Direction::XPos => [1.0, 0.0, 0.0],
            Direction::XNeg => [-1.0, 0.0, 0.0],
            Direction::YPos => [0.0, 1.0, 0.0],
            Direction::YNeg => [0.0, -1.0, 0.0],
            Direction::ZPos => [0.0, 0.0, 1.0],
            Direction::ZNeg => [0.0, 0.0, -1.0],
            Direction::None => [0.0, 0.0, 0.0],
        }
    }
}

impl From<[i32; 3]> for Direction {
    fn from(v: [i32; 3]) -> Self {
        match v {
            [1, 0, 0] => Direction::XPos,
            [-1, 0, 0] => Direction::XNeg,
            [0, 1, 0] => Direction::YPos,
            [0, -1, 0] => Direction::YNeg,
            [0, 0, 1] => Direction::ZPos,
            [0, 0, -1] => Direction::ZNeg,
            _ => Direction::None,
        }
    }
}

pub fn map_from_dir_and_world_pos(dir: Direction, pos: [f32; 3]) -> [f32; 2] {
    match dir {
        Direction::XPos => [pos[1], pos[2]],
        Direction::XNeg => [pos[1], pos[2]],
        Direction::YPos => [pos[0], pos[2]],
        Direction::YNeg => [pos[0], pos[2]],
        Direction::ZPos => [pos[0], pos[1]],
        Direction::ZNeg => [pos[0], pos[1]],
        Direction::None => [0.0, 0.0],
    }
}

pub fn map_from_dir_and_local_pos(dir: Direction, pos: [f32; 2], mut world_pos: [f32; 3]) -> [f32; 3] {
    match dir {
        Direction::XPos | Direction::XNeg => {
            world_pos[1] = pos[0];
            world_pos[2] = pos[1];
        }
        Direction::YPos | Direction::YNeg => {
            world_pos[0] = pos[0];
            world_pos[2] = pos[1];
        }
        Direction::ZPos | Direction::ZNeg => {
            world_pos[0] = pos[0];
            world_pos[1] = pos[1];
        }
        Direction::None => {}
    };
    world_pos
}

/// Maps a neighbour direction from a face of the planet tree to the neighboring face and gives a transformation of neighbor descent directions.
fn map_to_neighbor(from_dir: Direction, dir: [i32; 2]) -> (Direction, NeighborTransform) {
    let to_dir = match from_dir {
        Direction::XPos => match dir {
            [-1, _] => Direction::YNeg,
            [1, _] => Direction::YPos,
            [_, -1] => Direction::ZNeg,
            [_, 1] => Direction::ZPos,
            _ => Direction::None,
        },
        Direction::XNeg => match dir {
            [-1, _] => Direction::YNeg,
            [1, _] => Direction::YPos,
            [_, -1] => Direction::ZNeg,
            [_, 1] => Direction::ZPos,
            _ => Direction::None,
        },
        Direction::YPos => match dir {
            [-1, _] => Direction::XNeg,
            [1, _] => Direction::XPos,
            [_, -1] => Direction::ZNeg,
            [_, 1] => Direction::ZPos,
            _ => Direction::None,
        },
        Direction::YNeg => match dir {
            [-1, _] => Direction::XNeg,
            [1, _] => Direction::XPos,
            [_, -1] => Direction::ZNeg,
            [_, 1] => Direction::ZPos,
            _ => Direction::None,
        },
        Direction::ZPos => match dir {
            [-1, _] => Direction::XNeg,
            [1, _] => Direction::XPos,
            [_, -1] => Direction::YNeg,
            [_, 1] => Direction::YPos,
            _ => Direction::None,
        },
        Direction::ZNeg => match dir {
            [-1, _] => Direction::XNeg,
            [1, _] => Direction::XPos,
            [_, -1] => Direction::YNeg,
            [_, 1] => Direction::YPos,
            _ => Direction::None,
        },
        Direction::None => Direction::None,
    };
    (to_dir, transform_from_dir_to_dir(from_dir, to_dir))
}

pub fn map_neighbor_index(from_dir: Direction, to_dir: Direction, index: usize) -> usize {
    let dir = neighbor_dir_from_index::<2>(index);
    let new_dir = map_from_dir_to_dir(from_dir, to_dir, dir);
    neighbor_index(new_dir).unwrap()
}

fn map_from_dir_to_dir(from_dir: Direction, to_dir: Direction, dir: [i32; 2]) -> [i32; 2] {
    if from_dir != to_dir {        
        match transform_from_dir_to_dir(from_dir, to_dir) {
            NeighborTransform::Mirror { axis } => mirror(axis, dir),
            NeighborTransform::Rotate { clockwise } => simple_rotate(clockwise, dir),
            NeighborTransform::RotateMirror { clockwise, axis } => mirror(axis, simple_rotate(clockwise, dir)), 
            NeighborTransform::None => dir,
        }
    } else {
        dir
    }
}

fn map_from_dir_to_dir_float(from_dir: Direction, to_dir: Direction, dir: [f32; 2]) -> [f32; 2] {
    if from_dir != to_dir {        
        match transform_from_dir_to_dir(from_dir, to_dir) {
            NeighborTransform::Mirror { axis } => mirror_float(axis, dir),
            NeighborTransform::Rotate { clockwise } => simple_rotate_float(clockwise, dir),
            NeighborTransform::RotateMirror { clockwise, axis } => mirror_float(axis, simple_rotate_float(clockwise, dir)), 
            NeighborTransform::None => dir,
        }
    } else {
        dir
    }   
}

fn transform_from_dir_to_dir(from_dir: Direction, to_dir: Direction) -> NeighborTransform {
    match from_dir {
        Direction::XPos => match to_dir {            
            Direction::XNeg => NeighborTransform::Mirror { axis: 0 },
            Direction::YPos => NeighborTransform::Mirror { axis: 0 },
            Direction::ZNeg => NeighborTransform::RotateMirror { clockwise: true, axis: 1},
            Direction::ZPos => NeighborTransform::Rotate { clockwise: false },
            _ => NeighborTransform::None,
        },
        Direction::XNeg => match to_dir {
            Direction::XPos => NeighborTransform::Mirror { axis: 0 },
            Direction::YNeg => NeighborTransform::Mirror { axis: 0 },            
            Direction::ZNeg => NeighborTransform::Rotate { clockwise: false },
            Direction::ZPos => NeighborTransform::RotateMirror { clockwise: true, axis: 1 },
            _ => NeighborTransform::None,
        },
        Direction::YPos => match to_dir {            
            Direction::XPos => NeighborTransform::Mirror { axis: 0 },            
            Direction::YNeg => NeighborTransform::Mirror { axis: 1 },
            Direction::ZPos => NeighborTransform::Mirror { axis: 1 },
            _ => NeighborTransform::None,
        },
        Direction::YNeg => match to_dir {
            Direction::XNeg => NeighborTransform::Mirror { axis: 0 },
            Direction::YPos => NeighborTransform::Mirror { axis: 1 },            
            Direction::ZNeg => NeighborTransform::Mirror { axis: 1 },            
            _ => NeighborTransform::None,
        },
        Direction::ZPos => match to_dir {
            Direction::XNeg => NeighborTransform::RotateMirror { clockwise: true, axis: 1 },
            Direction::XPos => NeighborTransform::Rotate { clockwise: true },            
            Direction::YPos => NeighborTransform::Mirror { axis: 1 },
            Direction::ZNeg => NeighborTransform::Mirror { axis: 1 },
            _ => NeighborTransform::None,
        },
        Direction::ZNeg => match to_dir {
            Direction::XNeg => NeighborTransform::Rotate { clockwise: true },
            Direction::XPos => NeighborTransform::RotateMirror { clockwise: true, axis: 1 },
            Direction::YNeg => NeighborTransform::Mirror { axis: 1 },            
            Direction::ZPos => NeighborTransform::Mirror { axis: 1 },
            _ => NeighborTransform::None,
        },
        Direction::None => NeighborTransform::None,
    }
}

//Simple transforms for neighbor descent
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NeighborTransform {
    Mirror { axis: usize },
    Rotate { clockwise: bool },
    RotateMirror { clockwise: bool, axis: usize },
    None,
}

//Rotates by +-90deg rotation 
fn simple_rotate(clockwise: bool, mut coord: [i32; 2]) -> [i32; 2] {    
    if clockwise {
        coord[0] *= -1;        
    } else {
        coord[1] *= -1;
    }    
    [coord[1], coord[0]]
}

fn mirror(index: usize, mut coord: [i32; 2]) -> [i32; 2] {
    coord[index] *= -1;
    coord
}

fn simple_rotate_float(clockwise: bool, mut coord: [f32; 2]) -> [f32; 2] {    
    if clockwise {
        coord[0] *= -1.0;
    } else {
        coord[1] *= -1.0;
    }
    [coord[1], coord[0]]
}

fn mirror_float(index: usize, mut coord: [f32; 2]) -> [f32; 2] {
    coord[index] *= -1.0;
    coord
}
