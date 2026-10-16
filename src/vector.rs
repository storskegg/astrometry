//! Three-vectors and rotation matrices, as SOFA has them.

pub type Vec3 = [f64; 3];

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(k: f64, a: Vec3) -> Vec3 {
    [k * a[0], k * a[1], k * a[2]]
}

pub fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

pub fn unit(a: Vec3) -> Vec3 {
    scale(1.0 / norm(a), a)
}

/// A 3×3 matrix, rows first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [[f64; 3]; 3]);

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// Rotation of the axes about x by `a` radians, as SOFA's `iauRx` applies it.
    pub fn rx(a: f64) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]])
    }

    /// Rotation of the axes about z by `a` radians, as SOFA's `iauRz` applies it.
    pub fn rz(a: f64) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3([[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]])
    }

    /// `self` then `other`: the product other × self.
    pub fn then(self, other: Mat3) -> Mat3 {
        let (a, b) = (other.0, self.0);
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
            }
        }
        Mat3(m)
    }

    pub fn apply(&self, v: Vec3) -> Vec3 {
        let m = &self.0;
        [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
    }
}
