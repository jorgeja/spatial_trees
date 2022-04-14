use crate::NodeKey;

pub trait ChildBehaviour<const D: usize>
where
    Self: Boundary<D>,
{
    fn set_parent(&mut self, node_key: NodeKey);

    fn get_parent(&self) -> Option<NodeKey>;

    fn children(&self) -> Option<&[NodeKey]>;

    fn has_children(&self) -> bool;

    fn child_position_from_key(&self, child: NodeKey) -> Option<[i32; D]> {
        self.get_child_index(child).map(child_position::<D>)
    }

    fn get_child(&self, direction: [i32; D]) -> Option<NodeKey> {
        self.children().map(|children| {
            let index = direction.iter().enumerate().fold(0, |acc, (i, d)| {
                acc + d.clamp(&0, &1) * (i as i32 + 1 + (i as i32 - 1).clamp(0, 1))
            }) as usize;
            children[index]
        })
    }

    fn get_child_index(&self, child: NodeKey) -> Option<usize> {
        self.children().and_then(|children| {
            for (i, child_key) in children.iter().enumerate() {
                if child == *child_key {
                    return Some(i);
                }
            }
            None
        })
    }

    fn set_child_keys(&mut self, children: &[NodeKey]);

    fn take_children(&mut self) -> Vec<NodeKey>;
}

pub fn child_position<const D: usize>(mut child_index: usize) -> [i32; D] {
    let mut out = [0; D];
    out.iter_mut().enumerate().for_each(|(i, out)| {
        let v = child_index % ((i + 1) * 2);
        child_index -= v;
        *out = if v == 0 { -1 } else { 1 };
    });
    out
}

pub fn neighbor_index<const D: usize>(direction: [i32; D]) -> Option<usize> {
    let index = direction.iter().enumerate().fold(-1, |acc, (i, d)| {
        let v = match d {
            -1 => i * 2 + 1,
            1 => i * 2 + 2,
            _ => 0,
        } as i32;
        acc + v
    });

    if index != -1 {
        Some(index as usize)
    } else {
        None
    }
}

/// Finds all sub-children in a cartesian direction. The direction can only be +-1 along one axis in 2d or 3d
pub fn child_positions_in_direction<const D: usize>(direction: [i32; D]) -> Vec<[i32; D]> {
    let non_zero_index = direction
        .iter()
        .enumerate()
        .find(|(_, v)| **v != 0)
        .map(|v| v.0)
        .unwrap();
    let num_border_children = 2usize.pow(D as u32) / 2;
    let mut check_dirs = Vec::with_capacity(num_border_children);
    for mut child_index in 0..num_border_children {
        let mut check_dir = direction;
        for pos in 0..D - 1 {
            let v = child_index % ((pos + 1) * 2);
            child_index -= v;
            let v = if v == 0 { -1 } else { 1 };
            if pos < non_zero_index {
                check_dir[pos] = v;
            } else {
                check_dir[pos + 1] = v;
            }
        }
        check_dirs.push(check_dir)
    }
    check_dirs
}

pub fn all_neighbor_directions<const D: usize>() -> impl Iterator<Item = [i32; D]> + 'static {
    (0..D * 2).map(|i| {
        let v = i % 2;
        let v = if v == 0 { -1 } else { 1 };
        let mut out = [0; D];
        out[i % D] = v;
        out
    })
}

pub trait NeighborBehaviour<const D: usize>
where
    Self: Boundary<D>,
{
    fn neighbor_sizes(&mut self) -> &mut [f32];
}

pub trait Boundary<const D: usize> {
    fn from_bounds(size: f32, pos: [f32; D]) -> Self;
    fn pos(&self) -> [f32; D];
    fn size(&self) -> f32;
    fn contains_point(&self, pos: [f32; D]) -> bool {
        let half_size = self.size() / 2.0;

        pos.iter()
            .zip(self.pos().iter())
            .all(|(other, this)| *this - half_size < *other && *other < *this + half_size)
    }

    fn contains(&self, other: &Self) -> bool {
        let (min, max) = other.bounds();
        self.contains_point(min) && self.contains_point(max)
    }

    fn bounds(&self) -> ([f32; D], [f32; D]) {
        let half_size = self.size() / 2.0;
        let pos = self.pos();

        let mut min = [0.0; D];
        let mut max = [0.0; D];

        for i in 0..D {
            min[i] = pos[i] - half_size;
            max[i] = pos[i] + half_size;
        }

        (min, max)
    }
}
