//! Linear algebra for coordinate transformations
//!
//! This module provides basic linear algebra on points on the 2D plane:
//!
//! - You can construct matrices that represent basic 2d transformations like
//!   scaling, translation and rotation.
//!
//! - You can compose and invert matrices.
//!
//! - You can apply matrices to points.
//!
//! This module is only lightly typed: points are [f32; 2] values, and matrices
//! are column-major [[f32; 3]; 3] values. These are the types Glium works with
//! directly anyway.
//!
//! Transformations are represented as 3x3 matrices, using homogeneous
//! coordinates to represent translations as well as linear transformations.
//! When applied to an [f32; 2] point, the point is extended with an implicit
//! `1.0` to make it a homogeneous coordinate.

pub type Vector = [f32; 2];
pub type Matrix = [[f32; 3]; 3];

/// Return a matrix that scales a homogeneous 2D point by `sx` and `sy` along the
/// x and y axes.
pub fn scale_transform(sx: f32, sy: f32) -> Matrix {
    [[sx,  0.0, 0.0],
     [0.0, sy,  0.0],
     [0.0, 0.0, 1.0]]
}

/// Return a matrix that translates a homogeneous 2D point by `dx` and `dy` to
/// the right and upwards.
pub fn translate_transform(dx: f32, dy: f32) -> Matrix {
    [[1.0, 0.0, 0.0],
     [0.0, 1.0, 0.0],
     [dx,  dy,  1.0]]
}

// These sorts of modules usually also provide a rotation matrix, but we don't
// need rotation in rbattle.

/// A vector that can be extended to an [f32; 3] vector, and converted back.
/// On `[f32; 3]` vectors, these are the identity function.
pub trait Homogeneous {
    /// Convert `self` into homogeneous coordinates.
    fn into(self) -> [f32; 3];

    /// Convent a homogeneous vector to `self`.
    fn from([f32; 3]) -> Self;
}

impl Homogeneous for [f32; 2] {
    fn into(self) -> [f32; 3] { [self[0], self[1], 1.0] }
    fn from(h: [f32; 3]) -> Self { [h[0] / h[2], h[1] / h[2]] }
}

impl Homogeneous for [f32; 3] {
    fn into(self) -> [f32; 3] { self }
    fn from(h: [f32; 3]) -> Self { h }
}

/// Multiply each element in `vec` by `scalar`.
pub fn scale(vec: [f32; 3], scalar: f32) -> [f32; 3] {
    [vec[0] * scalar,
     vec[1] * scalar,
     vec[2] * scalar]
}

/// Divide each element in `vec` by `scalar`. This is equivalent to `scale(vec,
/// 1.0 / scalar)`, but might be more accurate.
pub fn scale_inv(vec: [f32; 3], scalar: f32) -> [f32; 3] {
    [vec[0] / scalar,
     vec[1] / scalar,
     vec[2] / scalar]
}

/// Add `lhs` to `rhs` elementwise.
pub fn add(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] + rhs[0],
     lhs[1] + rhs[1],
     lhs[2] + rhs[02]]
}

/// Compute the cross product or vector product of `lhs` and `rhs`.
pub fn cross(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [(lhs[1] * rhs[2]) - (lhs[2] * rhs[1]),
     (lhs[2] * rhs[0]) - (lhs[0] * rhs[2]),
     (lhs[0] * rhs[1]) - (lhs[1] * rhs[0])]
}

/// Compute the midpoint between the `lhs` and `rhs`.
pub fn midpoint(lhs: [f32; 2], rhs: [f32; 2]) -> [f32; 2] {
    [(lhs[0] + rhs[0]) / 2.0, (lhs[1] + rhs[1]) / 2.0]
}

/// Apply the transformation `trans` to `vec`. The `vec` argument may be either
/// an `[f32; 2]` or an `[f32; 3]`.
pub fn apply<V: Homogeneous>(trans: Matrix, vec: V) -> V {
    let h = vec.into();
    Homogeneous::from(add(add(scale(trans[0], h[0]),
                              scale(trans[1], h[1])),
                          scale(trans[2], h[2])))
}

/// Return a matrix that is the product of `lhs` and `rhs`. In other words,
/// return a matrix whose effects as a transformation are equivalent to first
/// applying `rhs` and then applying `lhs`.
pub fn compose(lhs: Matrix, rhs: Matrix) -> Matrix {
    [apply(lhs, rhs[0]),
     apply(lhs, rhs[1]),
     apply(lhs, rhs[2])]
}

/// Return a matrix whose n'th row is `m`'s n'th column.
pub fn transpose(m: Matrix) -> Matrix {
    [[m[0][0], m[1][0], m[2][0]],
     [m[0][1], m[1][1], m[2][1]],
     [m[0][2], m[1][2], m[2][2]]]
}

/// Return the determinant of `m`.
pub fn determinant(m: Matrix) -> f32 {
    (m[0][0] * (m[1][1] * m[2][2] - m[2][1] * m[1][2]) -
     m[1][0] * (m[0][1] * m[2][2] - m[2][1] * m[0][2]) +
     m[2][0] * (m[0][1] * m[1][2] - m[1][1] * m[0][2]))
}

/// Return the inverse of `m`. In other words, return a matrix that undoes
/// whatever transformation `m` does. Some matrices have no inverse; for those,
/// this function returns `None`.
pub fn inverse(m: Matrix) -> Option<Matrix> {
    let det = determinant(m);
    if det == 0.0 {
        None
    } else {
        Some(transpose([scale_inv(cross(m[1], m[2]), det),
                        scale_inv(cross(m[2], m[0]), det),
                        scale_inv(cross(m[0], m[1]), det)]))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_scale_transform() {
        assert_eq!(apply(scale_transform(2.0, 3.0), [5.0, 7.0]),
                   [10.0, 21.0]);
    }

    #[test]
    fn test_translate_transform() {
        assert_eq!(apply(translate_transform(1.0, 10.0), [100.0, 1000.0]),
                   [101.0, 1010.0]);
    }

    #[test]
    fn test_compose() {
        let scale = scale_transform(2.0, 3.0);
        let translate = translate_transform(1.0, 10.0);
        assert_eq!(apply(compose(translate, scale), [5.0, 7.0]),
                   [11.0, 31.0]);
        assert_eq!(apply(compose(scale, translate), [5.0, 7.0]),
                   [12.0, 51.0]);
    }

    #[test]
    fn test_inverse() {
        let scale = scale_transform(2.0, 8.0);
        assert_eq!(inverse(scale),
                   Some(scale_transform(0.5, 0.125)));

        let translate = translate_transform(1.0, 10.0);
        assert_eq!(inverse(translate),
                   Some(translate_transform(-1.0, -10.0)));

        assert_eq!(inverse(compose(scale, translate)),
                   Some(compose(inverse(translate).unwrap(),
                                inverse(scale).unwrap())));

        assert_eq!(inverse(compose(translate, scale)),
                   Some(compose(inverse(scale).unwrap(),
                                inverse(translate).unwrap())));
    }
}
