mod node_traits;
mod tree_traits;
mod ntree;
mod oct_tree_node;
mod planet_tree_impl;
mod planet_tree_node;
mod quad_tree_node;


use slotmap::new_key_type;
new_key_type! {pub struct NodeKey;}

pub mod planet_tree {
    pub use crate::node_traits::*;
    pub use crate::tree_traits::*;  
    pub use crate::planet_tree_impl::*;
    pub use crate::planet_tree_node::PlanetTreeNode;
}

pub mod quad_tree {
    pub use crate::node_traits::*;
    pub use crate::tree_traits::*;    
    pub type QuadTree = crate::ntree::NTree<QuadTreeNode, 2>;
    pub use crate::quad_tree_node::QuadTreeNode;    
}

pub mod oct_tree {
    pub use crate::node_traits::*;
    pub use crate::tree_traits::*;
    pub type OctTree = crate::ntree::NTree<OctTreeNode, 3>;
    pub use crate::oct_tree_node::OctTreeNode;    
}