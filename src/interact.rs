//! Pure view coordinate transforms; pointer movement never clones the document.
use crate::MindMapError;

/// A point in CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}
/// Per-view viewport, excluded from document serialization and history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Translation in screen CSS pixels.
    pub translation: Point,
    /// Positive magnification (bounded to 0.1–4).
    pub scale: f64,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            translation: Point::default(),
            scale: 1.0,
        }
    }
}
impl Viewport {
    /// Convert view-local screen coordinates into unscaled canvas coordinates.
    pub fn to_canvas(self, point: Point) -> Result<Point, MindMapError> {
        self.validate()?;
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(MindMapError::InvalidGeometry);
        }
        let result = Point {
            x: (point.x - self.translation.x) / self.scale,
            y: (point.y - self.translation.y) / self.scale,
        };
        if !result.x.is_finite() || !result.y.is_finite() {
            return Err(MindMapError::InvalidGeometry);
        }
        Ok(result)
    }
    /// Zoom around a view-local anchor, preserving its canvas point.
    pub fn zoom_at(&mut self, scale: f64, anchor: Point) -> Result<(), MindMapError> {
        let canvas = self.to_canvas(anchor)?;
        if !scale.is_finite() {
            return Err(MindMapError::InvalidGeometry);
        }
        let scale = scale.clamp(0.1, 4.0);
        let translation = Point {
            x: anchor.x - canvas.x * scale,
            y: anchor.y - canvas.y * scale,
        };
        if !translation.x.is_finite() || !translation.y.is_finite() {
            return Err(MindMapError::InvalidGeometry);
        }
        self.translation = translation;
        self.scale = scale;
        Ok(())
    }
    /// Translate by a finite screen-space delta.
    pub fn pan(&mut self, delta: Point) -> Result<(), MindMapError> {
        self.validate()?;
        let translation = Point {
            x: self.translation.x + delta.x,
            y: self.translation.y + delta.y,
        };
        if !translation.x.is_finite() || !translation.y.is_finite() {
            return Err(MindMapError::InvalidGeometry);
        }
        self.translation = translation;
        Ok(())
    }
    /// Center a canvas-space point in the specified view dimensions.
    pub fn center(&mut self, target: Point, size: Point) -> Result<(), MindMapError> {
        self.validate()?;
        if ![target.x, target.y, size.x, size.y]
            .into_iter()
            .all(f64::is_finite)
            || size.x <= 0.0
            || size.y <= 0.0
        {
            return Err(MindMapError::InvalidGeometry);
        }
        let translation = Point {
            x: size.x / 2.0 - target.x * self.scale,
            y: size.y / 2.0 - target.y * self.scale,
        };
        if !translation.x.is_finite() || !translation.y.is_finite() {
            return Err(MindMapError::InvalidGeometry);
        }
        self.translation = translation;
        Ok(())
    }
    /// Check public fields before applying a transform.
    pub fn validate(self) -> Result<(), MindMapError> {
        if [self.translation.x, self.translation.y, self.scale]
            .into_iter()
            .all(f64::is_finite)
            && (0.1..=4.0).contains(&self.scale)
        {
            Ok(())
        } else {
            Err(MindMapError::InvalidGeometry)
        }
    }
}
