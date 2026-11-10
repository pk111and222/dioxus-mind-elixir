//! Renderer-owned mounted APIs keep platform glue out of component logic.
use dioxus::prelude::MountedData;

use crate::{MindMapError, generate_branch::Rect};

pub(crate) async fn measure(element: &MountedData) -> Result<Rect, MindMapError> {
    let rect = element
        .get_client_rect()
        .await
        .map_err(|error| MindMapError::Measurement(error.to_string()))?;
    let rect = Rect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    };
    if !rect.is_valid() {
        return Err(MindMapError::InvalidGeometry);
    }
    Ok(rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::html::{
        MountedResult, RenderedElementBacking,
        geometry::{
            PixelsRect,
            euclid::{Point2D, Size2D},
        },
    };
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    struct InvalidElement;
    impl RenderedElementBacking for InvalidElement {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn get_client_rect(
            &self,
        ) -> std::pin::Pin<Box<dyn Future<Output = MountedResult<PixelsRect>>>> {
            Box::pin(async {
                Ok(PixelsRect::new(
                    Point2D::new(f64::NAN, 0.0),
                    Size2D::new(10.0, 20.0),
                ))
            })
        }
    }
    #[test]
    fn unsupported_renderer_returns_typed_measurement_error() {
        let mounted = MountedData::new(());
        let mut future = pin!(measure(&mounted));
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(MindMapError::Measurement(_)))
        ));
    }
    #[test]
    fn invalid_renderer_geometry_is_rejected() {
        let mounted = MountedData::new(InvalidElement);
        let mut future = pin!(measure(&mounted));
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(MindMapError::InvalidGeometry))
        );
    }
}
