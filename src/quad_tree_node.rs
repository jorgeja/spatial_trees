use crate::{node_traits::*, NodeKey};

#[derive(Debug)]
pub struct QuadTreeNode {
    pub size: f32,
    pub pos: [f32; 2],
    pub neighbor_sizes: [f32; 4],
    pub parent: Option<NodeKey>,
    pub children: Option<[NodeKey; 4]>,
}

impl Boundary<2> for QuadTreeNode {
    fn from_bounds(size: f32, pos: [f32; 2]) -> Self {
        Self {
            size,
            pos,
            neighbor_sizes: [-1.0; 4],
            parent: None,
            children: None,
        }
    }

    fn pos(&self) -> [f32; 2] {
        self.pos
    }

    fn size(&self) -> f32 {
        self.size
    }
}

impl ChildBehaviour<2> for QuadTreeNode {
    fn set_parent(&mut self, node_key: NodeKey) {
        self.parent = Some(node_key);
    }

    fn get_parent(&self) -> Option<NodeKey> {
        self.parent
    }

    fn children(&self) -> Option<&[NodeKey]> {
        self.children.as_ref().map(|c| c.as_slice())
    }

    fn has_children(&self) -> bool {
        self.children.is_some()
    }

    fn set_child_keys(&mut self, children: &[NodeKey]) {
        self.children = Some([children[0], children[1], children[2], children[3]]);
    }

    fn take_children(&mut self) -> Vec<NodeKey> {
        if let Some(children) = self.children.take() {
            Vec::from_iter(children.into_iter())
        } else {
            vec![]
        }
    }
}

impl NeighborBehaviour<2> for QuadTreeNode {
    fn neighbor_sizes(&mut self) -> &mut [f32] {
        self.neighbor_sizes.as_mut_slice()
    }
}
