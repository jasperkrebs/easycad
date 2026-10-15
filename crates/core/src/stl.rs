//! Binary STL: the simplest way to take a shape to any slicer or CAD program.
//!
//! Layout: an 80-byte header, the triangle count, then per triangle its normal and three
//! corners as little-endian 32-bit floats and a 2-byte attribute (zero). Units are mm.

use crate::surface::Mesh;

/// The mesh as a binary STL file. `name` goes in the header (trimmed to 80 bytes).
pub fn binary(mesh: &Mesh, name: &str) -> Vec<u8> {
    let triangles: Vec<[u32; 3]> = mesh
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let mut out = Vec::with_capacity(84 + triangles.len() * 50);
    let mut header = [b' '; 80];
    for (slot, byte) in header.iter_mut().zip(
        format!("EasyCAD {name}")
            .bytes()
            .filter(|b| b.is_ascii_graphic() || *b == b' '),
    ) {
        *slot = byte;
    }
    out.extend_from_slice(&header);
    out.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    let vertex = |i: u32| -> [f32; 3] {
        let k = i as usize * 3;
        match mesh.positions.get(k..k + 3) {
            Some(p) => [p[0], p[1], p[2]],
            None => [0.0; 3],
        }
    };
    for t in triangles {
        let (a, b, c) = (vertex(t[0]), vertex(t[1]), vertex(t[2]));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let n = if len > 0.0 {
            [n[0] / len, n[1] / len, n[2] / len]
        } else {
            [0.0; 3]
        };
        for f in n.iter().chain(&a).chain(&b).chain(&c) {
            out.extend_from_slice(&f.to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_header_count_and_triangles() {
        let mesh = Mesh {
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            indices: vec![0, 1, 2],
        };
        let bytes = binary(&mesh, "test");
        assert_eq!(bytes.len(), 84 + 50);
        assert!(bytes.starts_with(b"EasyCAD test"));
        assert_eq!(
            u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]),
            1
        );
        // The normal of a counter-clockwise triangle in the XY plane points up (+Z).
        let nz = f32::from_le_bytes([bytes[92], bytes[93], bytes[94], bytes[95]]);
        assert_eq!(nz, 1.0);
        // Bad indices give a degenerate triangle, not a crash.
        let broken = Mesh {
            positions: vec![],
            indices: vec![0, 5, 9],
        };
        assert_eq!(binary(&broken, "x").len(), 134);
    }
}
