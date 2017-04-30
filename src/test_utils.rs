/// Utilities for tests.

use graph::Node;
use visible_graph::{Point,IndexedSegment};

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;
use std::ops::Range;

/// If `left` and `right` hold the same elements, ignoring order and repetition,
/// return `None`. Otherwise, return `Some(left_only, right_only)`, where
/// `left_only` and `right_only` are vectors of the elements appearing only one
/// one side or the other.
pub fn diff_elements<T: Clone + Hash + Eq>(left: &[T], right: &[T])
    -> Option<(HashSet<T>, HashSet<T>)>
{
    let left: HashSet<_> = FromIterator::from_iter(left.iter().cloned());
    let right: HashSet<_> = FromIterator::from_iter(right.iter().cloned());
    if left == right {
        None
    } else {
        Some((HashSet::from_iter(left.difference(&right).cloned()),
              HashSet::from_iter(right.difference(&left).cloned())))
    }
}

/// Take ownership of LEFT and RIGHT, and assert that they hold the same
/// elements, ignoring order and repetition.
macro_rules! assert_same_elements {
    ($left:expr, $right:expr) => ({
        let left = $left;
        let right = $right;
        if let Some((left_only, right_only)) = ::test_utils::diff_elements(&left, &right) {
            panic!("assertion failed: left and right have different elements:\n\
                    left has only `{:#?}`,\n\
                    right has only `{:#?}`",
                   left_only, right_only);
        }
    });
}


/// A newtype around visible_graph::Point that can be compared and hashed.
///
/// `f32` doesn't implement `Eq` or `Hash`, because a NaN != itself,
/// and `Eq` requires that `x == x`. This makes writing tests painful.
///
/// This newtype simply treats `NaN`s as equal, and hashes `f32` bitwise.
#[derive(Clone, Copy, Debug)]
pub struct EqPoint(pub Point);

/// Return the bit pattern of `f`.
fn f32_bits(f: f32) -> u32 {
    unsafe {
        // This is safe because every bit pattern is a valid `u32` value.
        ::std::mem::transmute::<f32, u32>(f)
    }
}

/// Bit-for-bit comparison on f32 values.
fn f32_eq(lhs: f32, rhs: f32) -> bool {
    // If neither are NaNs, then this behaves like equality on `f32` values
    // because floating-point comparison for equality checks every bit of a
    // non-NaN `f32` value.
    //
    // This will do random things with NaNs, but in our case one of the values
    // is always provided by a test case, which never pass NaNs, so it's
    // sufficient that no NaN compares equal to any non-NaN value.
    f32_bits(lhs) == f32_bits(rhs)
}

impl PartialEq<EqPoint> for EqPoint {
    fn eq(&self, other: &EqPoint) -> bool {
        f32_eq(self.0 .0, other.0 .0) &&
        f32_eq(self.0 .1, other.0 .1)
    }
}

impl Eq for EqPoint { }

impl Hash for EqPoint {
    fn hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        f32_bits(self.0 .0).hash(state);
        f32_bits(self.0 .1).hash(state);
    }
}

/// Convert a Vec<Point> to a Vec<EqPoint>.
pub fn into_eq_points(points: Vec<Point>) -> Vec<EqPoint> {
    unsafe {
        // This is safe because `EqPoint` is just a newtype for `Point`,
        // so they have the same representation in memory.
        ::std::mem::transmute::<Vec<Point>, Vec<EqPoint>>(points)
    }
}

/// An analogue to `IndexedSegment` that is easier to hash and compare in tests.
///
/// Whereas `IndexedSegment` just stores indices into a vector of endpoints,
/// SegmentWithPoints actually stores the coordinates. Also, the start and end
/// of `line` are ordered consistently.
///
/// Otherwise, the meanings of the fields are the same as those in `IndexedSegment`.
#[derive(Clone, Debug)]
pub struct SegmentWithPoints {
    line: Range<Point>,
    neighbor: Option<Node>
}

/// Return `segment` with its endpoints put in a consistent order.
fn order_segment(segment: &Range<Point>) -> Range<Point> {
    let start = &segment.start;
    let end = &segment.end;
    if f32_bits(start.0) > f32_bits(end.0) ||
        (f32_bits(start.0) == f32_bits(end.0) && f32_bits(start.1) > f32_bits(end.1))
    {
        Range { start: segment.end, end: segment.start }
    } else {
        segment.clone()
    }
}

impl SegmentWithPoints {
    pub fn new(line: &Range<Point>, neighbor: Option<Node>) -> SegmentWithPoints {
        SegmentWithPoints {
            line: order_segment(line),
            neighbor: neighbor
        }
    }

    /// Construct a `SegmentWithPoints` from an `IndexedSegment` and a slice of
    /// the endpoints it refers to.
    pub fn from_indexed(indexed: &IndexedSegment, endpoints: &[Point]) -> SegmentWithPoints {
        let line = Range {
            start: endpoints[indexed.line.start],
            end:   endpoints[indexed.line.end]
        };
        SegmentWithPoints::new(&line, indexed.neighbor)
    }
}

/// Given a `segments` and `endpoints`, a slice of endpoints the segments refer
/// to, construct an equivalent `Vec<SegmentWithPoints>`.
pub fn into_points(segments: &[IndexedSegment], endpoints: &[Point]) -> Vec<SegmentWithPoints> {
    segments.iter()
        .map(|seg| SegmentWithPoints::from_indexed(seg, endpoints))
        .collect::<Vec<_>>()
}

impl PartialEq<SegmentWithPoints> for SegmentWithPoints {
    fn eq(&self, other: &SegmentWithPoints) -> bool {
        (EqPoint(self.line.start) == EqPoint(other.line.start) &&
         EqPoint(self.line.end) == EqPoint(other.line.end) &&
         self.neighbor == other.neighbor)
    }
}

impl Eq for SegmentWithPoints { }

impl Hash for SegmentWithPoints {
    fn hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        let line = order_segment(&self.line);
        EqPoint(line.start).hash(state);
        EqPoint(line.end).hash(state);
        self.neighbor.hash(state);
    }
}
