//! Lists the nodes of a model that have an identifier, with where each is in the
//! model's rest pose: `nodes <file.insm>`. The game's code refers to nodes by these
//! identifiers (a hand, a muzzle, the spot a collision sphere hangs on).

use anyhow::{Context, Result};
use brs_formats::insm;

/// `v` turned by the quaternion `q` (x, y, z, w).
fn turned(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let (u, w) = ([q[0], q[1], q[2]], q[3]);
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v).map(|c| c * 2.0);
    let extra = cross(u, t);
    [v[0] + w * t[0] + extra[0], v[1] + w * t[1] + extra[1], v[2] + w * t[2] + extra[2]]
}

/// The product of two quaternions: `b` first, then `a`.
fn times(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn main() -> Result<()> {
    let path = std::env::args().nth(1).context("usage: nodes <file.insm>")?;
    let data = std::fs::read(&path).with_context(|| format!("reading {path}"))?;
    let model = insm::Model::parse(&data)?;
    let mut world: Vec<([f32; 3], [f32; 4])> = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        let (at, facing) = match node.parent {
            Some(parent) => {
                let (parent_at, parent_facing) = world[parent];
                let moved = turned(parent_facing, node.translation);
                ([parent_at[0] + moved[0], parent_at[1] + moved[1], parent_at[2] + moved[2]], times(parent_facing, node.rotation))
            }
            None => (node.translation, node.rotation),
        };
        world.push((at, facing));
        if node.id != 0xff {
            println!("node {index:3}  id {:3} ({:#04x})  parent {:?}  at ({:8.2}, {:8.2}, {:8.2})", node.id, node.id, node.parent, at[0], at[1], at[2]);
        }
    }
    println!("{} nodes", model.nodes.len());
    Ok(())
}
