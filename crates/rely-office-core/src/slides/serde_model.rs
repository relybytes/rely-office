// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The JSON of the slide model's enums whose variants hold a bare value, each tagged by `type`
//! like the rest of the model: a tagged object cannot hold a bare string or list.

use serde::{Deserialize, Serialize};

use super::{Bullet, Gradient, Paint, ShapeGeometry};
use crate::geometry::ShapePath;

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum ShapeGeometryJson {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Line { flipped: bool },
    Path { paths: Vec<ShapePath> },
    Other,
}

impl From<ShapeGeometry> for ShapeGeometryJson {
    fn from(geometry: ShapeGeometry) -> Self {
        match geometry {
            ShapeGeometry::Rectangle => Self::Rectangle,
            ShapeGeometry::RoundedRectangle => Self::RoundedRectangle,
            ShapeGeometry::Ellipse => Self::Ellipse,
            ShapeGeometry::Line { flipped } => Self::Line { flipped },
            ShapeGeometry::Path(paths) => Self::Path { paths },
            ShapeGeometry::Other => Self::Other,
        }
    }
}

impl From<ShapeGeometryJson> for ShapeGeometry {
    fn from(geometry: ShapeGeometryJson) -> Self {
        match geometry {
            ShapeGeometryJson::Rectangle => Self::Rectangle,
            ShapeGeometryJson::RoundedRectangle => Self::RoundedRectangle,
            ShapeGeometryJson::Ellipse => Self::Ellipse,
            ShapeGeometryJson::Line { flipped } => Self::Line { flipped },
            ShapeGeometryJson::Path { paths } => Self::Path(paths),
            ShapeGeometryJson::Other => Self::Other,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum PaintJson {
    Solid {
        colour: String,
    },
    Gradient {
        gradient: Gradient,
    },
    Picture {
        media: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile_emu: Option<(i64, i64)>,
    },
}

impl From<Paint> for PaintJson {
    fn from(paint: Paint) -> Self {
        match paint {
            Paint::Solid(colour) => Self::Solid { colour },
            Paint::Gradient(gradient) => Self::Gradient { gradient },
            Paint::Picture { media, tile_emu } => Self::Picture { media, tile_emu },
        }
    }
}

impl From<PaintJson> for Paint {
    fn from(paint: PaintJson) -> Self {
        match paint {
            PaintJson::Solid { colour } => Self::Solid(colour),
            PaintJson::Gradient { gradient } => Self::Gradient(gradient),
            PaintJson::Picture { media, tile_emu } => Self::Picture { media, tile_emu },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum BulletJson {
    Character { text: String },
    Number { text: String },
}

impl From<Bullet> for BulletJson {
    fn from(bullet: Bullet) -> Self {
        match bullet {
            Bullet::Character(text) => Self::Character { text },
            Bullet::Number(text) => Self::Number { text },
        }
    }
}

impl From<BulletJson> for Bullet {
    fn from(bullet: BulletJson) -> Self {
        match bullet {
            BulletJson::Character { text } => Self::Character(text),
            BulletJson::Number { text } => Self::Number(text),
        }
    }
}

#[cfg(test)]
mod tests;
