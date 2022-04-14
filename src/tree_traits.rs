use crate::{node_traits::*, NodeKey};

use ahash::AHashMap as HashMap;

pub trait NodeStorage {
    type NodeType;
    type NodeKeyType;

    fn get_node(&self, node_key: Self::NodeKeyType) -> Option<&Self::NodeType>;
    fn get_mut_node(&mut self, node_key: Self::NodeKeyType) -> Option<&mut Self::NodeType>;
    fn get_node_unchecked(&self, node_key: Self::NodeKeyType) -> &Self::NodeType;
    fn get_mut_node_unchecked(&mut self, node_key: Self::NodeKeyType) -> &mut Self::NodeType;
    fn insert_node(&mut self, node: Self::NodeType) -> Self::NodeKeyType;
    fn remove_node(&mut self, node_key: Self::NodeKeyType) -> Option<Self::NodeType>;
}

pub trait TreeBehaviour<const D: usize>
where
    Self: NodeStorage<NodeKeyType = NodeKey>,
    <Self as NodeStorage>::NodeType: Boundary<D> + ChildBehaviour<D> + NeighborBehaviour<D>,
{
    fn insert(&mut self, f: impl Fn(&Self::NodeType) -> bool) -> Vec<TreeEvent> {
        let mut events = vec![];
        let mut pending_node_keys = self.root_items();
        while let Some(node_key) = pending_node_keys.pop() {
            if f(self.get_node_unchecked(node_key)) {
                let node = &self.get_node_unchecked(node_key);
                if let Some(children) = node.children() {
                    pending_node_keys.extend(children.iter());
                } else if node.size() > self.min_size() {
                    let parent_pos = node.pos();
                    let new_children = self.create_children(node_key);
                    self.grow_event(&mut events, parent_pos, node_key, &new_children);
                    pending_node_keys.extend(new_children.iter());
                };
            } else {
                self.shrink_event(&mut events, node_key);
            }
        }
        events
    }

    fn create_children(&mut self, parent_key: NodeKey) -> Vec<NodeKey> {
        let (parent_size, parent_pos) = {
            let parent = self.get_node_unchecked(parent_key);
            (parent.size(), parent.pos())
        };

        let new_size = parent_size / 2.0;
        let quart_size = parent_size / 4.0;

        let mut new_child_indexes = vec![];
        let num_children = 2usize.pow(D as u32);

        for child_index in 0..num_children {
            let pos = child_position::<D>(child_index);
            let mut child_pos = parent_pos;
            child_pos.iter_mut().zip(pos.iter()).for_each(|(out, p)| {
                let v = *out + *p as f32 * quart_size;
                *out = v;
            });

            let mut child = Self::NodeType::from_bounds(new_size, child_pos);
            child.set_parent(parent_key);
            new_child_indexes.push(self.insert_node(child));
        }
        self.get_mut_node_unchecked(parent_key)
            .set_child_keys(new_child_indexes.as_slice());
        new_child_indexes
    }

    fn remove_children_recursively(&mut self, parent_key: NodeKey) -> Vec<NodeKey> {
        let mut removed_nodes = vec![];
        let mut pending_node_keys = self.get_mut_node_unchecked(parent_key).take_children();
        while let Some(node_key) = pending_node_keys.pop() {
            if let Some(mut node) = self.remove_node(node_key) {
                let children = node.take_children();
                if !children.is_empty() {
                    pending_node_keys.extend(children);
                } else {
                    removed_nodes.push(node_key);
                }
            }
        }

        removed_nodes
    }

    fn grow_event(
        &self,
        events: &mut Vec<TreeEvent>,
        pos: [f32; D],
        parent_key: NodeKey,
        new_children: &[NodeKey],
    ) {
        for event in events.iter_mut().rev() {
            if let TreeEvent::Grown { parent, children } = event {
                if self.get_node_unchecked(*parent).contains_point(pos) {
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

    fn shrink_event(&mut self, events: &mut Vec<TreeEvent>, parent_key: NodeKey) {
        let removed_nodes = self.remove_children_recursively(parent_key);
        if !removed_nodes.is_empty() {
            events.push(TreeEvent::Shrunk {
                retained: parent_key,
                removed: removed_nodes,
            })
        }
    }

    fn min_size(&self) -> f32;
    fn root_items(&self) -> Vec<NodeKey>;
}

pub trait TreeNeighbourBehaviour<const D: usize>
where
    Self: NodeStorage<NodeKeyType = NodeKey> + TreeBehaviour<D>,
    <Self as NodeStorage>::NodeType: Boundary<D> + ChildBehaviour<D> + NeighborBehaviour<D>,
{
    //Finds all bordering neighbors in a direction away from the node
    fn get_neighbors(&self, node_key: NodeKey, direction: [i32; D]) -> Vec<Self::NodeKeyType> {
        let (mut node, neighbor_descents) = match self.find_shared_parent(node_key, direction) {
            Some((node, descent)) => (node, descent),
            None => return vec![],
        };

        // Find neighbor of same size or larger
        node = self.neighbour_descent(node, neighbor_descents);
        if !self.get_node_unchecked(node).has_children() {
            return vec![node];
        }

        // Find all bordering nodes smaller than the subject node
        self.bordering_neighbours(node, direction)
    }

    // Find neighbor of same size or larger
    fn neighbour_descent(
        &self,
        mut node_key: NodeKey,
        descents: Vec<[i32; D]>,
    ) -> Self::NodeKeyType {
        for nd in descents.iter().rev() {
            if let Some(child_node) = self.get_node_unchecked(node_key).get_child(*nd) {
                node_key = child_node;
                if !self.get_node_unchecked(node_key).has_children() {
                    break;
                }
            }
        }
        node_key
    }

    //Find the smaller neighbors bordering to a node, direction is from the subject
    fn bordering_neighbours(
        &self,
        neighbour_node_key: NodeKey,
        direction: [i32; D],
    ) -> Vec<NodeKey> {
        let mut child_direction = direction;
        child_direction.iter_mut().for_each(|e| *e *= -1);
        let child_directions = child_positions_in_direction(child_direction);

        let mut pending_nodes = vec![neighbour_node_key];
        let mut neighbors = vec![];
        while let Some(pending_node_key) = pending_nodes.pop() {
            let node = self.get_node_unchecked(pending_node_key);
            if node.has_children() {
                for child_direction in &child_directions {
                    if let Some(c) = node.get_child(*child_direction) {
                        pending_nodes.push(c);
                    }
                }
            } else {
                neighbors.push(pending_node_key);
            }
        }
        neighbors
    }

    fn find_shared_parent(
        &self,
        node_key: NodeKey,
        direction: [i32; D],
    ) -> Option<(NodeKey, Vec<[i32; D]>)> {
        let mut node = node_key;
        let mut working_direction = direction;
        let mut neighbor_descents = vec![];

        //Find the shared parent between the node and the potentioal node in the neighbor direction.
        while let Some(parent) = self.get_node_unchecked(node).get_parent() {
            if working_direction.iter().all(|v| *v == 0) {
                break;
            }

            let node_descent = self
                .get_node_unchecked(parent)
                .child_position_from_key(node)
                .unwrap();

            let mut neighbor_descent = [0; D];
            neighbor_descent
                .iter_mut()
                .zip(node_descent.iter().zip(working_direction.iter()))
                .for_each(|(out, (nd, dir))| *out = *nd * (1 - 2 * dir.abs()));

            neighbor_descents.push(neighbor_descent);

            working_direction
                .iter_mut()
                .zip(node_descent.iter())
                .for_each(|(wd, nd)| *wd = (*nd + *wd) / 2);

            node = parent;
        }

        // if direction is not [0, 0] then there is no shared parent between the node and the potential node in the neighbor direction.
        if !working_direction.iter().all(|v| *v == 0) {
            None
        } else {
            Some((node, neighbor_descents))
        }
    }

    fn update_neighbor_sizes(
        &mut self,
        node_key: NodeKey,
        visited_nodes: &mut HashMap<NodeKey, NeighborSizeEvent>,
    ) {
        let mut neighbor_sizes = vec![];
        let node_size = self.get_node_unchecked(node_key).size();

        for direction in all_neighbor_directions::<D>() {
            let mut opposite_dir = direction;
            opposite_dir.iter_mut().for_each(|e| *e *= -1);

            for neighbour_key in self.get_neighbors(node_key, direction) {
                if self.update_neighbor_size(neighbour_key, node_size, opposite_dir)
                    == NeighborSizeEvent::ChangedSize
                {
                    visited_nodes
                        .entry(neighbour_key)
                        .or_insert(NeighborSizeEvent::ChangedSize);
                }

                let neighbour_size = self.get_node_unchecked(neighbour_key).size();

                if neighbour_size < node_size {
                    neighbor_sizes.push((direction, node_size));
                } else {
                    neighbor_sizes.push((direction, neighbour_size));
                }
            }
        }

        let child_node = self.get_mut_node_unchecked(node_key);
        for (dir, size) in neighbor_sizes.iter() {
            let index = neighbor_index(*dir).unwrap();
            child_node.neighbor_sizes()[index] = *size;
        }
        visited_nodes.insert(node_key, NeighborSizeEvent::New);
    }

    // updates the border size in the neighbor node with the correct size. Direction is the direction of the border from the neighbors point of view
    fn update_neighbor_size(
        &mut self,
        neighbour_key: NodeKey,
        subject_size: f32,
        direction: [i32; D],
    ) -> NeighborSizeEvent {
        let neighbour = self.get_mut_node_unchecked(neighbour_key);
        let neighbor_size = neighbour.size();
        if let Some(neighbor_size_index) = neighbor_index::<D>(direction) {
            let neighbors_border_size = &mut neighbour.neighbor_sizes()[neighbor_size_index];

            if *neighbors_border_size != subject_size {
                if neighbor_size < subject_size {
                    *neighbors_border_size = subject_size;
                    return NeighborSizeEvent::ChangedSize;
                } else if *neighbors_border_size != neighbor_size {
                    *neighbors_border_size = neighbor_size;
                    return NeighborSizeEvent::ChangedSize;
                }
            }
        }

        NeighborSizeEvent::None
    }

    fn insert_and_update_neighbors(
        &mut self,
        f: impl Fn(&Self::NodeType) -> bool,
    ) -> Vec<TreeEvent> {
        let mut events = self.insert(f);
        self.update_neighbors_from_events(&mut events);
        events
    }

    fn update_neighbors_from_events(&mut self, events: &mut Vec<TreeEvent>) {
        let mut visited_nodes = HashMap::new();
        for event in events.iter() {
            match event {
                TreeEvent::Grown {
                    parent: _,
                    children,
                } => {
                    for child in children {
                        self.update_neighbor_sizes(*child, &mut visited_nodes);
                    }
                }
                TreeEvent::Shrunk {
                    retained,
                    removed: _,
                } => self.update_neighbor_sizes(*retained, &mut visited_nodes),
                _ => {}
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NeighborSizeEvent {
    ChangedSize,
    New,
    None,
}

#[derive(Debug, Clone)]
pub enum TreeEvent {
    Grown {
        parent: NodeKey,
        children: Vec<NodeKey>,
    },
    Shrunk {
        retained: NodeKey,
        removed: Vec<NodeKey>,
    },
    NeighborSizesChanged(NodeKey),
}
