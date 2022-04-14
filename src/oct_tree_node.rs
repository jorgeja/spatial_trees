use crate::{node_traits::*, NodeKey};

#[derive(Debug)]
pub struct OctTreeNode {
    pub size: f32,
    pub pos: [f32; 3],
    pub neighbor_sizes: [f32; 6],
    pub parent: Option<NodeKey>,
    pub children: Option<[NodeKey; 8]>,
}

impl Boundary<3> for OctTreeNode {
    fn from_bounds(size: f32, pos: [f32; 3]) -> Self {
        Self {
            size,
            pos,
            neighbor_sizes: [-1.0; 6],
            parent: None,
            children: None,
        }
    }

    fn pos(&self) -> [f32; 3] {
        self.pos
    }

    fn size(&self) -> f32 {
        self.size
    }
}

impl ChildBehaviour<3> for OctTreeNode {
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
        self.children = Some([
            children[0],
            children[1],
            children[2],
            children[3],
            children[4],
            children[5],
            children[6],
            children[7],
        ]);
    }

    fn take_children(&mut self) -> Vec<NodeKey> {
        if let Some(children) = self.children.take() {
            Vec::from_iter(children.into_iter())
        } else {
            vec![]
        }
    }
}

impl NeighborBehaviour<3> for OctTreeNode {
    fn neighbor_sizes(&mut self) -> &mut [f32] {
        self.neighbor_sizes.as_mut_slice()
    }
}
