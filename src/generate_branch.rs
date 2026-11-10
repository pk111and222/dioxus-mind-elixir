/// A rectangle in viewport CSS pixels, independent of any renderer type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn is_valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            && self.width > 0.0
            && self.height > 0.0
    }
}

/// Convert a measured viewport rectangle to unscaled canvas coordinates.
pub(crate) fn local(rect: Rect, canvas: Rect, scale: f64) -> Rect {
    Rect {
        x: (rect.x - canvas.x) / scale,
        y: (rect.y - canvas.y) / scale,
        width: rect.width / scale,
        height: rect.height / scale,
    }
}

/// Main-branch quadratic and rounded vertical paths, following pinned upstream.
/// Sub-branches use edge-center cubic paths, avoiding upstream's text underlining
/// and its dependency on a separately parsed CSS gap.
pub(crate) fn branch(
    parent: Rect,
    child: Rect,
    direction: crate::LayoutDirection,
    main: bool,
    height: f64,
) -> String {
    use crate::LayoutDirection;
    if direction == LayoutDirection::Down {
        let (x1, y1, x2, y2) = (
            parent.x + parent.width / 2.0,
            parent.y + parent.height,
            child.x + child.width / 2.0,
            child.y,
        );
        if (x1 - x2).abs() < f64::EPSILON {
            return format!("M {x1} {y1} V {y2}");
        }
        let mid = (y1 + y2) / 2.0;
        // Radius derives from measured free space, bounded by the upstream 8px cap.
        let radius = 8.0_f64
            .min((x2 - x1).abs() / 2.0)
            .min((mid - y1).abs())
            .min((y2 - mid).abs());
        let sign = (x2 - x1).signum();
        return format!(
            "M {x1} {y1} V {} Q {x1} {mid} {} {mid} H {} Q {x2} {mid} {x2} {} V {y2}",
            mid - radius,
            x1 + sign * radius,
            x2 - sign * radius,
            mid + radius
        );
    }
    let left = direction == LayoutDirection::Left;
    let y1 = parent.y + parent.height / 2.0;
    let y2 = child.y + child.height / 2.0;
    let x2 = if left { child.x + child.width } else { child.x };
    if main {
        let fraction = ((y2 - y1).abs() / height.max(1.0)).min(1.0);
        let offset = parent.width / 10.0 + (1.0 - fraction) * 0.25 * parent.width / 2.0;
        let x1 = parent.x + parent.width / 2.0 + if left { -offset } else { offset };
        format!("M {x1} {y1} Q {x1} {y2} {x2} {y2}")
    } else {
        let x1 = if left {
            parent.x
        } else {
            parent.x + parent.width
        };
        let middle = (x1 + x2) / 2.0;
        format!("M {x1} {y1} C {middle} {y1} {middle} {y2} {x2} {y2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_geometry_and_four_direction_anchors() {
        let canvas = Rect {
            x: 100.0,
            y: 50.0,
            width: 800.0,
            height: 500.0,
        };
        let measured = Rect {
            x: 140.0,
            y: 90.0,
            width: 160.0,
            height: 80.0,
        };
        let parent = local(measured, canvas, 2.0);
        assert_eq!(
            parent,
            Rect {
                x: 20.0,
                y: 20.0,
                width: 80.0,
                height: 40.0
            }
        );
        let child = Rect {
            x: 200.0,
            y: 100.0,
            width: 100.0,
            height: 30.0,
        };
        assert_eq!(
            branch(parent, child, crate::LayoutDirection::Right, false, 500.0),
            "M 100 40 C 150 40 150 115 200 115"
        );
        let left = Rect { x: -150.0, ..child };
        assert_eq!(
            branch(parent, left, crate::LayoutDirection::Left, false, 500.0),
            "M 20 40 C -15 40 -15 115 -50 115"
        );
        assert!(
            branch(parent, child, crate::LayoutDirection::Down, true, 500.0).ends_with("V 100")
        );
        let straight = Rect {
            x: 10.0,
            y: 100.0,
            width: 100.0,
            height: 30.0,
        };
        assert_eq!(
            branch(parent, straight, crate::LayoutDirection::Down, false, 500.0),
            "M 60 60 V 100"
        );
        assert!(
            branch(parent, child, crate::LayoutDirection::Right, true, 500.0).ends_with("200 115")
        );
        assert!(
            branch(parent, left, crate::LayoutDirection::Left, true, 500.0).ends_with("-50 115")
        );
    }

    #[test]
    fn invalid_geometry_is_not_drawn() {
        assert!(
            !Rect {
                x: f64::NAN,
                y: 0.0,
                width: 20.0,
                height: 10.0
            }
            .is_valid()
        );
        assert!(
            !Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 10.0
            }
            .is_valid()
        );
    }
}
