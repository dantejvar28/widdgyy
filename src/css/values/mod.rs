// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed CSS value modules.
//!
//! Every value is parsed once when the stylesheet is applied and resolved
//! against a [`ResolveContext`] during layout. Nothing here allocates per
//! frame.

pub mod angle;
pub mod color;
pub mod gradient;
pub mod length;
pub mod shadow;
pub mod transform;

pub use angle::Angle;
pub use color::{Color, parse as parse_color};
pub use gradient::{ColorStop, Gradient, GradientKind};
pub use length::{Axis, Calc, Edges, Length, ResolveContext, Size, Unit};
pub use shadow::Shadow;
pub use transform::TransformOp;
