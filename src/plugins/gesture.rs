//! Platform-independent pointer hit testing; coordinates are CSS client pixels.
use crate::{LayoutDirection, MindMapError, Placement, Point};

/// Rectangle measured in client coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}
impl HitRect {
    /// Reject invalid measurements before hit testing.
    pub fn validate(self) -> Result<(), MindMapError> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(())
        } else {
            Err(MindMapError::InvalidGeometry)
        }
    }
    /// Whether the point is inside the rectangle.
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
    /// Whether the rectangle intersects a selection area, regardless of direction.
    pub fn intersects(self, start: Point, end: Point) -> bool {
        self.x <= start.x.max(end.x)
            && self.x + self.width >= start.x.min(end.x)
            && self.y <= start.y.max(end.y)
            && self.y + self.height >= start.y.min(end.y)
    }
}
/// The upstream five client-pixel threshold avoids moving nodes on clicks.
pub fn is_drag(start: Point, current: Point) -> bool {
    (current.x - start.x).hypot(current.y - start.y) >= 5.0
}
/// Resolve before/after bands (12 × zoom) and the central child target.
/// LEFT/RIGHT/SIDE siblings are vertical; DOWN siblings are horizontal.
pub fn drop_placement(
    id: &str,
    rect: HitRect,
    at: Point,
    layout: LayoutDirection,
    scale: f64,
) -> Result<Placement, MindMapError> {
    rect.validate()?;
    if !scale.is_finite() || scale <= 0.0 || !at.x.is_finite() || !at.y.is_finite() {
        return Err(MindMapError::InvalidGeometry);
    }
    let (offset, length) = if layout == LayoutDirection::Down {
        (at.x - rect.x, rect.width)
    } else {
        (at.y - rect.y, rect.height)
    };
    let band = (12.0 * scale).min(length / 3.0);
    Ok(if offset < band {
        Placement::Before(id.into())
    } else if offset > length - band {
        Placement::After(id.into())
    } else {
        Placement::In(id.into())
    })
}
/// Per-pointer-step edge pan within a 50px band, independent of document history.
pub fn edge_pan(bounds: HitRect, at: Point) -> Result<Point, MindMapError> {
    bounds.validate()?;
    if !at.x.is_finite() || !at.y.is_finite() {
        return Err(MindMapError::InvalidGeometry);
    }
    let axis = |v: f64, low: f64, length: f64| {
        if v < low + 50.0 {
            ((low + 50.0 - v) / 50.0).clamp(0.0, 1.0) * 10.0
        } else if v > low + length - 50.0 {
            -((v - (low + length - 50.0)) / 50.0).clamp(0.0, 1.0) * 10.0
        } else {
            0.0
        }
    };
    Ok(Point {
        x: axis(at.x, bounds.x, bounds.width),
        y: axis(at.y, bounds.y, bounds.height),
    })
}
/// Viewport that fits the canvas-space box `a..b` into a view of `size`,
/// centred, with a 24px margin and magnification at most 100%.
pub fn fit(a: Point, b: Point, size: Point) -> Result<crate::Viewport, MindMapError> {
    let (w, h) = ((b.x - a.x).abs(), (b.y - a.y).abs());
    if ![a.x, a.y, b.x, b.y, size.x, size.y]
        .into_iter()
        .all(f64::is_finite)
        || size.x <= 0.0
        || size.y <= 0.0
    {
        return Err(MindMapError::InvalidGeometry);
    }
    let margin = 24.0;
    let sx = (size.x - margin * 2.0).max(1.0) / w.max(1.0);
    let sy = (size.y - margin * 2.0).max(1.0) / h.max(1.0);
    let mut view = crate::Viewport {
        translation: Point::default(),
        scale: sx.min(sy).clamp(0.1, 1.0),
    };
    view.center(
        Point {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        },
        size,
    )?;
    Ok(view)
}
