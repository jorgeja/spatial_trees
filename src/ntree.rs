use crate::{node_traits::*, tree_traits::*, NodeKey};
use slotmap::SlotMap;

/// Shared struct between 2d QuadTree and 3d OctTree.
pub struct NTree<T, const D: usize>
where
    T: ChildBehaviour<D> + NeighborBehaviour<D> + Boundary<D>,
{
    pub nodes: SlotMap<NodeKey, T>,
    pub min_size: f32,
    root: NodeKey,
}

impl<T, const D: usize> NTree<T, D>
where
    T: ChildBehaviour<D> + NeighborBehaviour<D> + Boundary<D>,
{
    pub fn new(min_size: f32, size: f32, pos: [f32; D]) -> Self {
        let mut nodes = SlotMap::default();
        let root = nodes.insert(T::from_bounds(size, pos));

        Self {
            min_size,
            nodes,
            root,
        }
    }

    pub fn iter_leaf_nodes(&self) -> impl Iterator<Item = (NodeKey, &T)> {
        self.nodes.iter().filter(|(_, node)| !node.has_children())
    }
}

impl<T, const D: usize> TreeBehaviour<D> for NTree<T, D>
where
    T: ChildBehaviour<D> + NeighborBehaviour<D> + Boundary<D>,
{
    fn min_size(&self) -> f32 {
        self.min_size
    }

    fn root_items(&self) -> Vec<NodeKey> {
        vec![self.root]
    }
}

impl<T, const D: usize> NodeStorage for NTree<T, D>
where
    T: ChildBehaviour<D> + NeighborBehaviour<D> + Boundary<D>,
{
    type NodeType = T;
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

impl<T, const D: usize> TreeNeighbourBehaviour<D> for NTree<T, D> where
    T: Boundary<D> + ChildBehaviour<D> + NeighborBehaviour<D>
{
}
