// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The preset shapes of `DrawingML` that `presets.rs` does not write in guide language, as
//! `OpenDocument` enhanced geometries: the modifiers with their `DrawingML` names, one
//! `draw:equation` formula a line (`?fN` is the Nth), the view of each subpath when it has its
//! own, and the `draw:enhanced-path`. Generated from the `oox-drawingml-cs-presets` table of
//! `LibreOffice` (MPL-2.0), itself drawn from the preset definitions of ECMA-376.

pub(super) const ENHANCED_PRESETS: &str = r"
[accentBorderCallout1]
mod adj1=18750 adj2=-8333 adj3=112500 adj4=-38333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f4 0 L ?f4 ?f7 L 0 ?f7 Z N M ?f1 0 L ?f1 ?f7 F N M ?f1 ?f0 L ?f3 ?f2 F N

[accentBorderCallout2]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=112500 adj6=-46667
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f6 0 L ?f6 ?f9 L 0 ?f9 Z N M ?f1 0 L ?f1 ?f9 F N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 F N

[accentBorderCallout3]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=100000 adj6=-16667 adj7=112963 adj8=-8333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logheight*$6 /100000
eq logwidth*$7 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f8 0 L ?f8 ?f11 L 0 ?f11 Z N M ?f1 0 L ?f1 ?f11 F N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 L ?f7 ?f6 F N

[accentCallout1]
mod adj1=18750 adj2=-8333 adj3=112500 adj4=-38333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f4 0 L ?f4 ?f7 L 0 ?f7 Z S N M ?f1 0 L ?f1 ?f7 F N M ?f1 ?f0 L ?f3 ?f2 F N

[accentCallout2]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=112500 adj6=-46667
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f6 0 L ?f6 ?f9 L 0 ?f9 Z S N M ?f1 0 L ?f1 ?f9 F N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 F N

[accentCallout3]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=100000 adj6=-16667 adj7=112963 adj8=-8333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logheight*$6 /100000
eq logwidth*$7 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f8 0 L ?f8 ?f11 L 0 ?f11 Z S N M ?f1 0 L ?f1 ?f11 F N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 L ?f7 ?f6 F N

[actionButtonBackPrevious]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq logwidth
eq logheight
path M 0 0 L ?f8 0 L ?f8 ?f9 L 0 ?f9 Z M ?f6 ?f2 L ?f7 ?f3 L ?f7 ?f4 Z S N M ?f6 ?f2 L ?f7 ?f3 L ?f7 ?f4 Z H S N M ?f6 ?f2 L ?f7 ?f3 L ?f7 ?f4 Z F N M 0 0 L ?f8 0 L ?f8 ?f9 L 0 ?f9 Z F N

[actionButtonBeginning]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq ?f0 *3/4
eq ?f8 *1/8
eq ?f8 *1/4
eq ?f6 +?f9 -0
eq ?f6 +?f10 -0
eq logwidth
eq logheight
path M 0 0 L ?f13 0 L ?f13 ?f14 L 0 ?f14 Z M ?f12 ?f2 L ?f7 ?f3 L ?f7 ?f4 Z M ?f11 ?f3 L ?f6 ?f3 L ?f6 ?f4 L ?f11 ?f4 Z S N M ?f12 ?f2 L ?f7 ?f3 L ?f7 ?f4 Z M ?f11 ?f3 L ?f6 ?f3 L ?f6 ?f4 L ?f11 ?f4 Z H S N M ?f12 ?f2 L ?f7 ?f3 L ?f7 ?f4 Z M ?f11 ?f3 L ?f11 ?f4 L ?f6 ?f4 L ?f6 ?f3 Z F N M 0 0 L ?f13 0 L ?f13 ?f14 L 0 ?f14 Z F N

[actionButtonBlank]
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f0 0 L ?f0 ?f3 L 0 ?f3 Z N

[actionButtonDocument]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq ?f0 *9/32
eq logwidth/2
eq ?f6 +0-?f5
eq ?f6 +?f5 -0
eq ?f0 *3/16
eq ?f8 +0-?f9
eq ?f3 +?f9 -0
eq logwidth
eq logheight
path M 0 0 L ?f12 0 L ?f12 ?f13 L 0 ?f13 Z M ?f7 ?f3 L ?f10 ?f3 L ?f8 ?f11 L ?f8 ?f4 L ?f7 ?f4 Z S N M ?f7 ?f3 L ?f10 ?f3 L ?f10 ?f11 L ?f8 ?f11 L ?f8 ?f4 L ?f7 ?f4 Z I S N M ?f10 ?f3 L ?f10 ?f11 L ?f8 ?f11 Z H S N M ?f7 ?f3 L ?f10 ?f3 L ?f8 ?f11 L ?f8 ?f4 L ?f7 ?f4 Z M ?f8 ?f11 L ?f10 ?f11 L ?f10 ?f3 F N M 0 0 L ?f12 0 L ?f12 ?f13 L 0 ?f13 Z F N

[actionButtonEnd]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq ?f0 *3/4
eq ?f8 *3/4
eq ?f8 *7/8
eq ?f6 +?f9 -0
eq ?f6 +?f10 -0
eq logwidth
eq logheight
path M 0 0 L ?f13 0 L ?f13 ?f14 L 0 ?f14 Z M ?f11 ?f2 L ?f6 ?f3 L ?f6 ?f4 Z M ?f12 ?f3 L ?f7 ?f3 L ?f7 ?f4 L ?f12 ?f4 Z S N M ?f11 ?f2 L ?f6 ?f3 L ?f6 ?f4 Z M ?f12 ?f3 L ?f7 ?f3 L ?f7 ?f4 L ?f12 ?f4 Z H S N M ?f11 ?f2 L ?f6 ?f4 L ?f6 ?f3 Z M ?f12 ?f3 L ?f7 ?f3 L ?f7 ?f4 L ?f12 ?f4 Z F N M 0 0 L ?f13 0 L ?f13 ?f14 L 0 ?f14 Z F N

[actionButtonForwardNext]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq logwidth
eq logheight
path M 0 0 L ?f8 0 L ?f8 ?f9 L 0 ?f9 Z M ?f7 ?f2 L ?f6 ?f3 L ?f6 ?f4 Z S N M ?f7 ?f2 L ?f6 ?f3 L ?f6 ?f4 Z H S N M ?f7 ?f2 L ?f6 ?f4 L ?f6 ?f3 Z F N M 0 0 L ?f8 0 L ?f8 ?f9 L 0 ?f9 Z F N

[actionButtonHelp]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq logwidth/2
eq ?f4 +0-?f1
eq ?f0 *3/4
eq ?f6 *1/7
eq ?f6 *3/14
eq ?f6 *2/7
eq ?f6 *3/7
eq ?f6 *4/7
eq ?f6 *17/28
eq ?f6 *21/28
eq ?f6 *11/14
eq ?f3 +?f9 -0
eq ?f3 +?f12 -0
eq ?f3 +?f13 -0
eq ?f3 +?f14 -0
eq ?f5 +?f8 -0
eq ?f5 +?f10 -0
eq ?f5 +?f11 -0
eq ?f6 *1/14
eq ?f6 *3/28
eq logwidth
eq logheight
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (-10800000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (-10800000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (-10800000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
path M 0 0 L ?f24 0 L ?f24 ?f25 L 0 ?f25 Z M ?f19 ?f15 G ?f9 ?f9 ?f26 ?f27 G ?f7 ?f8 ?f28 ?f29 G ?f22 ?f23 ?f30 ?f31 L ?f21 ?f17 L ?f20 ?f17 L ?f20 ?f16 G ?f7 ?f8 ?f32 ?f33 G ?f22 ?f23 ?f34 ?f35 G ?f7 ?f7 ?f36 ?f37 Z M ?f4 ?f18 G ?f23 ?f23 ?f38 ?f39 Z S N M ?f19 ?f15 G ?f9 ?f9 ?f40 ?f41 G ?f7 ?f8 ?f42 ?f43 G ?f22 ?f23 ?f44 ?f45 L ?f21 ?f17 L ?f20 ?f17 L ?f20 ?f16 G ?f7 ?f8 ?f46 ?f47 G ?f22 ?f23 ?f48 ?f49 G ?f7 ?f7 ?f50 ?f51 Z M ?f4 ?f18 G ?f23 ?f23 ?f52 ?f53 Z H S N M ?f19 ?f15 G ?f9 ?f9 ?f54 ?f55 G ?f7 ?f8 ?f56 ?f57 G ?f22 ?f23 ?f58 ?f59 L ?f21 ?f17 L ?f20 ?f17 L ?f20 ?f16 G ?f7 ?f8 ?f60 ?f61 G ?f22 ?f23 ?f62 ?f63 G ?f7 ?f7 ?f64 ?f65 Z M ?f4 ?f18 G ?f23 ?f23 ?f66 ?f67 Z F N M 0 0 L ?f24 0 L ?f24 ?f25 L 0 ?f25 Z F N

[actionButtonHome]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq ?f0 *3/4
eq ?f8 *1/16
eq ?f8 *1/8
eq ?f8 *3/16
eq ?f8 *5/16
eq ?f8 *7/16
eq ?f8 *9/16
eq ?f8 *11/16
eq ?f8 *3/4
eq ?f8 *13/16
eq ?f8 *7/8
eq ?f3 +?f9 -0
eq ?f3 +?f11 -0
eq ?f3 +?f12 -0
eq ?f3 +?f16 -0
eq ?f6 +?f10 -0
eq ?f6 +?f13 -0
eq ?f6 +?f14 -0
eq ?f6 +?f15 -0
eq ?f6 +?f17 -0
eq ?f6 +?f18 -0
eq logwidth
eq logheight
path M 0 0 L ?f29 0 L ?f29 ?f30 L 0 ?f30 Z M ?f5 ?f3 L ?f6 ?f2 L ?f23 ?f2 L ?f23 ?f4 L ?f28 ?f4 L ?f28 ?f2 L ?f7 ?f2 L ?f27 ?f21 L ?f27 ?f19 L ?f26 ?f19 L ?f26 ?f20 Z S N M ?f27 ?f21 L ?f27 ?f19 L ?f26 ?f19 L ?f26 ?f20 Z M ?f23 ?f2 L ?f23 ?f4 L ?f24 ?f4 L ?f24 ?f22 L ?f25 ?f22 L ?f25 ?f4 L ?f28 ?f4 L ?f28 ?f2 Z I S N M ?f5 ?f3 L ?f6 ?f2 L ?f7 ?f2 Z M ?f24 ?f22 L ?f25 ?f22 L ?f25 ?f4 L ?f24 ?f4 Z H S N M ?f5 ?f3 L ?f26 ?f20 L ?f26 ?f19 L ?f27 ?f19 L ?f27 ?f21 L ?f7 ?f2 L ?f28 ?f2 L ?f28 ?f4 L ?f23 ?f4 L ?f23 ?f2 L ?f6 ?f2 Z M ?f26 ?f20 L ?f27 ?f21 M ?f28 ?f2 L ?f23 ?f2 M ?f24 ?f4 L ?f24 ?f22 L ?f25 ?f22 L ?f25 ?f4 F N M 0 0 L ?f29 0 L ?f29 ?f30 L 0 ?f30 Z F N

[actionButtonInformation]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq logwidth/2
eq ?f4 +0-?f1
eq ?f0 *3/4
eq ?f6 *1/32
eq ?f6 *5/16
eq ?f6 *3/8
eq ?f6 *13/32
eq ?f6 *19/32
eq ?f6 *11/16
eq ?f6 *13/16
eq ?f6 *7/8
eq ?f3 +?f7 -0
eq ?f3 +?f8 -0
eq ?f3 +?f9 -0
eq ?f3 +?f13 -0
eq ?f3 +?f14 -0
eq ?f5 +?f8 -0
eq ?f5 +?f10 -0
eq ?f5 +?f11 -0
eq ?f5 +?f12 -0
eq ?f6 *3/32
eq logwidth
eq logheight
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (16200000)/60000.0
eq (21600000)/60000.0
path M 0 0 L ?f25 0 L ?f25 ?f26 L 0 ?f26 Z M ?f4 ?f3 G ?f1 ?f1 ?f27 ?f28 Z S N M ?f4 ?f3 G ?f1 ?f1 ?f29 ?f30 Z M ?f4 ?f15 G ?f24 ?f24 ?f31 ?f32 M ?f20 ?f16 L ?f20 ?f17 L ?f21 ?f17 L ?f21 ?f18 L ?f20 ?f18 L ?f20 ?f19 L ?f23 ?f19 L ?f23 ?f18 L ?f22 ?f18 L ?f22 ?f16 Z H S N M ?f4 ?f15 G ?f24 ?f24 ?f33 ?f34 M ?f20 ?f16 L ?f22 ?f16 L ?f22 ?f18 L ?f23 ?f18 L ?f23 ?f19 L ?f20 ?f19 L ?f20 ?f18 L ?f21 ?f18 L ?f21 ?f17 L ?f20 ?f17 Z J S N M ?f4 ?f3 G ?f1 ?f1 ?f35 ?f36 Z M ?f4 ?f15 G ?f24 ?f24 ?f37 ?f38 M ?f20 ?f16 L ?f22 ?f16 L ?f22 ?f18 L ?f23 ?f18 L ?f23 ?f19 L ?f20 ?f19 L ?f20 ?f18 L ?f21 ?f18 L ?f21 ?f17 L ?f20 ?f17 Z F N M 0 0 L ?f25 0 L ?f25 ?f26 L 0 ?f26 Z F N

[actionButtonMovie]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq ?f0 *3/4
eq ?f8 *1455/21600
eq ?f8 *1905/21600
eq ?f8 *2325/21600
eq ?f8 *16155/21600
eq ?f8 *17010/21600
eq ?f8 *19335/21600
eq ?f8 *19725/21600
eq ?f8 *20595/21600
eq ?f8 *5280/21600
eq ?f8 *5730/21600
eq ?f8 *6630/21600
eq ?f8 *7492/21600
eq ?f8 *9067/21600
eq ?f8 *9555/21600
eq ?f8 *13342/21600
eq ?f8 *14580/21600
eq ?f8 *15592/21600
eq ?f6 +?f9 -0
eq ?f6 +?f10 -0
eq ?f6 +?f11 -0
eq ?f6 +?f12 -0
eq ?f6 +?f13 -0
eq ?f6 +?f14 -0
eq ?f6 +?f15 -0
eq ?f6 +?f16 -0
eq ?f3 +?f17 -0
eq ?f3 +?f18 -0
eq ?f3 +?f19 -0
eq ?f3 +?f20 -0
eq ?f3 +?f21 -0
eq ?f3 +?f22 -0
eq ?f3 +?f23 -0
eq ?f3 +?f24 -0
eq ?f3 +?f25 -0
eq ?f3 +?f26 -0
eq logwidth
eq logheight
path M 0 0 L ?f44 0 L ?f44 ?f45 L 0 ?f45 Z M ?f6 ?f34 L ?f6 ?f39 L ?f26 ?f39 L ?f27 ?f38 L ?f28 ?f38 L ?f28 ?f42 L ?f30 ?f42 L ?f30 ?f40 L ?f31 ?f40 L ?f33 ?f41 L ?f7 ?f41 L ?f7 ?f36 L ?f33 ?f36 L ?f32 ?f37 L ?f30 ?f37 L ?f30 ?f36 L ?f29 ?f35 L ?f27 ?f35 L ?f26 ?f34 Z S N M ?f6 ?f34 L ?f6 ?f39 L ?f26 ?f39 L ?f27 ?f38 L ?f28 ?f38 L ?f28 ?f42 L ?f30 ?f42 L ?f30 ?f40 L ?f31 ?f40 L ?f33 ?f41 L ?f7 ?f41 L ?f7 ?f36 L ?f33 ?f36 L ?f32 ?f37 L ?f30 ?f37 L ?f30 ?f36 L ?f29 ?f35 L ?f27 ?f35 L ?f26 ?f34 Z H S N M ?f6 ?f34 L ?f26 ?f34 L ?f27 ?f35 L ?f29 ?f35 L ?f30 ?f36 L ?f30 ?f37 L ?f32 ?f37 L ?f33 ?f36 L ?f7 ?f36 L ?f7 ?f41 L ?f33 ?f41 L ?f31 ?f40 L ?f30 ?f40 L ?f30 ?f42 L ?f28 ?f42 L ?f28 ?f38 L ?f27 ?f38 L ?f26 ?f39 L ?f6 ?f39 Z F N M 0 0 L ?f44 0 L ?f44 ?f45 L 0 ?f45 Z F N

[actionButtonReturn]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq ?f0 *3/4
eq ?f8 *7/8
eq ?f8 *3/4
eq ?f8 *5/8
eq ?f8 *3/8
eq ?f8 *1/4
eq ?f3 +?f10 -0
eq ?f3 +?f11 -0
eq ?f3 +?f13 -0
eq ?f6 +?f9 -0
eq ?f6 +?f10 -0
eq ?f6 +?f11 -0
eq ?f6 +?f12 -0
eq ?f6 +?f13 -0
eq ?f8 *1/8
eq logwidth
eq logheight
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
path M 0 0 L ?f23 0 L ?f23 ?f24 L 0 ?f24 Z M ?f7 ?f16 L ?f18 ?f3 L ?f5 ?f16 L ?f19 ?f16 L ?f19 ?f15 G ?f22 ?f22 ?f25 ?f26 L ?f20 ?f14 G ?f22 ?f22 ?f27 ?f28 L ?f21 ?f16 L ?f6 ?f16 L ?f6 ?f15 G ?f12 ?f12 ?f29 ?f30 L ?f5 ?f4 G ?f12 ?f12 ?f31 ?f32 L ?f17 ?f16 Z S N M ?f7 ?f16 L ?f18 ?f3 L ?f5 ?f16 L ?f19 ?f16 L ?f19 ?f15 G ?f22 ?f22 ?f33 ?f34 L ?f20 ?f14 G ?f22 ?f22 ?f35 ?f36 L ?f21 ?f16 L ?f6 ?f16 L ?f6 ?f15 G ?f12 ?f12 ?f37 ?f38 L ?f5 ?f4 G ?f12 ?f12 ?f39 ?f40 L ?f17 ?f16 Z H S N M ?f7 ?f16 L ?f17 ?f16 L ?f17 ?f15 G ?f12 ?f12 ?f41 ?f42 L ?f20 ?f4 G ?f12 ?f12 ?f43 ?f44 L ?f6 ?f16 L ?f21 ?f16 L ?f21 ?f15 G ?f22 ?f22 ?f45 ?f46 L ?f5 ?f14 G ?f22 ?f22 ?f47 ?f48 L ?f19 ?f16 L ?f5 ?f16 L ?f18 ?f3 Z F N M 0 0 L ?f23 0 L ?f23 ?f24 L 0 ?f24 Z F N

[actionButtonSound]
eq min(logwidth,logheight)
eq ?f0 *3/8
eq logheight/2
eq ?f2 +0-?f1
eq ?f2 +?f1 -0
eq logwidth/2
eq ?f5 +0-?f1
eq ?f5 +?f1 -0
eq ?f0 *3/4
eq ?f8 *1/8
eq ?f8 *5/16
eq ?f8 *5/8
eq ?f8 *11/16
eq ?f8 *3/4
eq ?f8 *7/8
eq ?f3 +?f9 -0
eq ?f3 +?f10 -0
eq ?f3 +?f12 -0
eq ?f3 +?f14 -0
eq ?f6 +?f10 -0
eq ?f6 +?f11 -0
eq ?f6 +?f13 -0
eq logwidth
eq logheight
path M 0 0 L ?f22 0 L ?f22 ?f23 L 0 ?f23 Z M ?f6 ?f16 L ?f6 ?f17 L ?f19 ?f17 L ?f20 ?f4 L ?f20 ?f3 L ?f19 ?f16 Z S N M ?f6 ?f16 L ?f6 ?f17 L ?f19 ?f17 L ?f20 ?f4 L ?f20 ?f3 L ?f19 ?f16 Z H S N M ?f6 ?f16 L ?f19 ?f16 L ?f20 ?f3 L ?f20 ?f4 L ?f19 ?f17 L ?f6 ?f17 Z M ?f21 ?f16 L ?f7 ?f15 M ?f21 ?f2 L ?f7 ?f2 M ?f21 ?f17 L ?f7 ?f18 F N M 0 0 L ?f22 0 L ?f22 ?f23 L 0 ?f23 Z F N

[bentArrow]
mod adj1=25000 adj2=25000 adj3=25000 adj4=43750
eq if(0-$1 ,0,if(50000-$1 ,$1 ,50000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq if(0-$2 ,0,if(50000-$2 ,$2 ,50000))
eq min(logwidth,logheight)
eq ?f4 *?f2 /100000
eq ?f4 *?f0 /100000
eq ?f5 *1/2
eq ?f6 +0-?f7
eq ?f4 *?f3 /100000
eq logwidth+0-?f9
eq logheight+0-?f8
eq min(?f10 ,?f11 )
eq 100000*?f12 /?f4
eq if(0-$3 ,0,if(?f13 -$3 ,$3 ,?f13 ))
eq ?f4 *?f14 /100000
eq ?f15 +0-?f5
eq max(?f16 ,0)
eq ?f5 +?f17 -0
eq logwidth+0-?f9
eq ?f8 +?f5 -0
eq ?f20 +?f8 -0
eq ?f8 +?f15 -0
eq ?f20 +?f17 -0
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
path M 0 ?f24 L 0 ?f22 G ?f15 ?f15 ?f26 ?f27 L ?f19 ?f8 L ?f19 0 L ?f25 ?f6 L ?f19 ?f21 L ?f19 ?f20 L ?f18 ?f20 G ?f17 ?f17 ?f28 ?f29 L ?f5 ?f24 Z N

[bentConnector2]
eq logwidth
eq logheight
path M 0 0 L ?f0 0 L ?f0 ?f1 F N

[bentConnector3]
mod adj1=50000
eq logwidth*$0 /100000
eq logheight/2
eq logwidth
eq logheight
path M 0 0 L ?f0 0 L ?f0 ?f3 L ?f2 ?f3 F N

[bentConnector4]
mod adj1=50000 adj2=50000
eq logwidth*$0 /100000
eq (?f0 +logwidth)/2
eq logheight*$1 /100000
eq (0+?f2 )/2
eq logwidth
eq logheight
path M 0 0 L ?f0 0 L ?f0 ?f2 L ?f4 ?f2 L ?f4 ?f5 F N

[bentConnector5]
mod adj1=50000 adj2=50000 adj3=50000
eq logwidth*$0 /100000
eq logwidth*$2 /100000
eq (?f0 +?f1 )/2
eq logheight*$1 /100000
eq (0+?f3 )/2
eq (logheight+?f3 )/2
eq logwidth
eq logheight
path M 0 0 L ?f0 0 L ?f0 ?f3 L ?f1 ?f3 L ?f1 ?f7 L ?f6 ?f7 F N

[bentUpArrow]
mod adj1=25000 adj2=25000 adj3=25000
eq if(0-$0 ,0,if(50000-$0 ,$0 ,50000))
eq if(0-$1 ,0,if(50000-$1 ,$1 ,50000))
eq if(0-$2 ,0,if(50000-$2 ,$2 ,50000))
eq min(logwidth,logheight)
eq ?f3 *?f2 /100000
eq ?f3 *?f1 /50000
eq logwidth+0-?f5
eq ?f3 *?f1 /100000
eq logwidth+0-?f7
eq ?f3 *?f0 /200000
eq ?f8 +0-?f9
eq ?f8 +?f9 -0
eq ?f3 *?f0 /100000
eq logheight+0-?f12
eq ?f11 *1/2
eq (?f13 +logheight)/2
eq (?f4 +logheight)/2
eq logheight
eq logwidth
path M 0 ?f13 L ?f10 ?f13 L ?f10 ?f4 L ?f6 ?f4 L ?f8 0 L ?f18 ?f4 L ?f11 ?f4 L ?f11 ?f17 L 0 ?f17 Z N

[bevel]
mod adj=12500
eq if(0-$0 ,0,if(50000-$0 ,$0 ,50000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq logwidth+0-?f2
eq logheight+0-?f2
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M ?f2 ?f2 L ?f3 ?f2 L ?f3 ?f4 L ?f2 ?f4 Z S N M 0 0 L ?f5 0 L ?f3 ?f2 L ?f2 ?f2 Z K S N M 0 ?f8 L ?f2 ?f4 L ?f3 ?f4 L ?f5 ?f8 Z I S N M 0 0 L ?f2 ?f2 L ?f2 ?f4 L 0 ?f8 Z J S N M ?f5 0 L ?f5 ?f8 L ?f3 ?f4 L ?f3 ?f2 Z H S N M 0 0 L ?f5 0 L ?f5 ?f8 L 0 ?f8 Z M ?f2 ?f2 L ?f3 ?f2 L ?f3 ?f4 L ?f2 ?f4 Z M 0 0 L ?f2 ?f2 M 0 ?f8 L ?f2 ?f4 M ?f5 0 L ?f3 ?f2 M ?f5 ?f8 L ?f3 ?f4 F N

[borderCallout3]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=100000 adj6=-16667 adj7=112963 adj8=-8333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logheight*$6 /100000
eq logwidth*$7 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f8 0 L ?f8 ?f11 L 0 ?f11 Z N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 L ?f7 ?f6 F N

[bracePair]
mod adj=8333
eq if(0-$0 ,0,if(25000-$0 ,$0 ,25000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq ?f1 *?f0 /50000
eq logwidth+0-?f3
eq logwidth+0-?f2
eq logheight/2
eq ?f6 +0-?f2
eq ?f6 +?f2 -0
eq logheight+0-?f2
eq ?f2 *29289/100000
eq ?f2 +?f10 -0
eq logwidth+0-?f11
eq logheight+0-?f10
eq logwidth/2
eq logheight
eq logwidth
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
path M ?f3 ?f15 G ?f2 ?f2 ?f17 ?f18 L ?f2 ?f8 G ?f2 ?f2 ?f19 ?f20 G ?f2 ?f2 ?f21 ?f22 L ?f2 ?f2 G ?f2 ?f2 ?f23 ?f24 L ?f4 0 G ?f2 ?f2 ?f25 ?f26 L ?f5 ?f7 G ?f2 ?f2 ?f27 ?f28 G ?f2 ?f2 ?f29 ?f30 L ?f5 ?f9 G ?f2 ?f2 ?f31 ?f32 Z S N M ?f3 ?f15 G ?f2 ?f2 ?f33 ?f34 L ?f2 ?f8 G ?f2 ?f2 ?f35 ?f36 G ?f2 ?f2 ?f37 ?f38 L ?f2 ?f2 G ?f2 ?f2 ?f39 ?f40 M ?f4 0 G ?f2 ?f2 ?f41 ?f42 L ?f5 ?f7 G ?f2 ?f2 ?f43 ?f44 G ?f2 ?f2 ?f45 ?f46 L ?f5 ?f9 G ?f2 ?f2 ?f47 ?f48 F N

[callout1]
mod adj1=18750 adj2=-8333 adj3=112500 adj4=-38333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f4 0 L ?f4 ?f7 L 0 ?f7 Z S N M ?f1 ?f0 L ?f3 ?f2 F N

[callout2]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=112500 adj6=-46667
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f6 0 L ?f6 ?f9 L 0 ?f9 Z S N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 F N

[callout3]
mod adj1=18750 adj2=-8333 adj3=18750 adj4=-16667 adj5=100000 adj6=-16667 adj7=112963 adj8=-8333
eq logheight*$0 /100000
eq logwidth*$1 /100000
eq logheight*$2 /100000
eq logwidth*$3 /100000
eq logheight*$4 /100000
eq logwidth*$5 /100000
eq logheight*$6 /100000
eq logwidth*$7 /100000
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
path M 0 0 L ?f8 0 L ?f8 ?f11 L 0 ?f11 Z S N M ?f1 ?f0 L ?f3 ?f2 L ?f5 ?f4 L ?f7 ?f6 F N

[can]
mod adj=25000
eq min(logwidth,logheight)
eq 50000*logheight/?f0
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq ?f0 *?f2 /200000
eq ?f3 +?f3 -0
eq logheight+0-?f3
eq logwidth/2
eq logheight/2
eq logheight
eq logwidth
eq logwidth/2
eq (10800000)/60000.0
eq (-10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
path M 0 ?f3 G ?f10 ?f3 ?f11 ?f12 L ?f9 ?f5 G ?f10 ?f3 ?f13 ?f14 Z S N M 0 ?f3 G ?f10 ?f3 ?f15 ?f16 G ?f10 ?f3 ?f17 ?f18 Z J S N M ?f9 ?f3 G ?f10 ?f3 ?f19 ?f20 G ?f10 ?f3 ?f21 ?f22 L ?f9 ?f5 G ?f10 ?f3 ?f23 ?f24 L 0 ?f3 F N

[chartPlus]
view 10 10 10 10
path M 5 0 L 5 10 M 0 5 L 10 5 F N M 0 0 L 0 10 L 10 10 L 10 0 Z S N

[chartStar]
view 10 10 10 10
path M 0 0 L 10 10 M 0 10 L 10 0 M 5 0 L 5 10 F N M 0 0 L 0 10 L 10 10 L 10 0 Z S N

[chartX]
view 10 10 10 10
path M 0 0 L 10 10 M 0 10 L 10 0 F N M 0 0 L 0 10 L 10 10 L 10 0 Z S N

[circularArrow]
mod adj1=12500 adj2=1142319 adj3=20457681 adj4=10800000 adj5=12500
eq if(0-$4 ,0,if(25000-$4 ,$4 ,25000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq if(1-$2 ,1,if(21599999-$2 ,$2 ,21599999))
eq if(0-$3 ,0,if(21599999-$3 ,$3 ,21599999))
eq min(logwidth,logheight)
eq ?f5 *?f2 /100000
eq ?f5 *?f0 /100000
eq ?f6 *1/2
eq logwidth/2
eq ?f9 +?f8 -?f7
eq logheight/2
eq ?f11 +?f8 -?f7
eq ?f10 +0-?f6
eq ?f12 +0-?f6
eq ?f13 +?f8 -0
eq ?f14 +?f8 -0
eq ?f15 *sin(pi*(?f3 )/10800000)
eq ?f16 *cos(pi*(?f3 )/10800000)
eq ?f15 *(cos(atan2(?f17 ,?f18 )))
eq ?f16 *(sin(atan2(?f17 ,?f18 )))
eq logwidth/2
eq ?f21 +?f19 -0
eq logheight/2
eq ?f23 +?f20 -0
eq min(?f13 ,?f14 )
eq ?f19 *?f19 /1
eq ?f20 *?f20 /1
eq ?f25 *?f25 /1
eq ?f26 +0-?f28
eq ?f27 +0-?f28
eq ?f29 *?f30 /?f26
eq ?f31 *1/?f27
eq 1+0-?f32
eq sqrt(?f33 )
eq ?f29 *1/?f19
eq ?f35 *1/?f20
eq (1+?f34 )/?f36
eq (10800000*atan2(?f37 ,1))/pi
eq ?f38 +21600000-0
eq if(?f38 ,?f38 ,?f39 )
eq ?f40 +0-?f3
eq ?f41 +21600000-0
eq if(?f41 ,?f41 ,?f42 )
eq ?f43 +0-10800000
eq ?f43 +0-21600000
eq if(?f44 ,?f45 ,?f43 )
eq abs(?f46 )
eq if(0-$1 ,0,if(?f47 -$1 ,$1 ,?f47 ))
eq ?f3 +?f48 -0
eq ?f15 *sin(pi*(?f49 )/10800000)
eq ?f16 *cos(pi*(?f49 )/10800000)
eq ?f15 *(cos(atan2(?f50 ,?f51 )))
eq ?f16 *(sin(atan2(?f50 ,?f51 )))
eq ?f21 +?f52 -0
eq ?f23 +?f53 -0
eq ?f10 *sin(pi*(?f4 )/10800000)
eq ?f12 *cos(pi*(?f4 )/10800000)
eq ?f10 *(cos(atan2(?f56 ,?f57 )))
eq ?f12 *(sin(atan2(?f56 ,?f57 )))
eq ?f21 +?f58 -0
eq ?f23 +?f59 -0
eq ?f7 *cos(pi*(?f49 )/10800000)
eq ?f7 *sin(pi*(?f49 )/10800000)
eq ?f22 +?f62 -0
eq ?f24 +?f63 -0
eq ?f7 *cos(pi*(?f49 )/10800000)
eq ?f7 *sin(pi*(?f49 )/10800000)
eq ?f22 +0-?f66
eq ?f24 +0-?f67
eq ?f68 +0-?f21
eq ?f69 +0-?f23
eq ?f64 +0-?f21
eq ?f65 +0-?f23
eq min(?f10 ,?f12 )
eq ?f70 *?f74 /?f10
eq ?f71 *?f74 /?f12
eq ?f72 *?f74 /?f10
eq ?f73 *?f74 /?f12
eq ?f77 +0-?f75
eq ?f78 +0-?f76
eq sqrt(?f79 *?f79 +?f80 *?f80 +0*0)
eq ?f75 *?f78 /1
eq ?f77 *?f76 /1
eq ?f82 +0-?f83
eq ?f74 *?f74 /1
eq ?f81 *?f81 /1
eq ?f85 *?f86 /1
eq ?f84 *?f84 /1
eq ?f87 +0-?f88
eq max(?f89 ,0)
eq sqrt(?f90 )
eq ?f80 *-1/1
eq if(?f92 ,-1,1)
eq ?f93 *?f79 /1
eq ?f94 *?f91 /1
eq ?f84 *?f80 /1
eq (?f96 +?f95 )/?f86
eq ?f96 +0-?f95
eq ?f98 *1/?f86
eq abs(?f80 )
eq ?f100 *?f91 /1
eq ?f84 *?f79 /-1
eq (?f102 +?f101 )/?f86
eq ?f102 +0-?f101
eq ?f104 *1/?f86
eq ?f77 +0-?f97
eq ?f77 +0-?f99
eq ?f78 +0-?f103
eq ?f78 +0-?f105
eq sqrt(?f106 *?f106 +?f108 *?f108 +0*0)
eq sqrt(?f107 *?f107 +?f109 *?f109 +0*0)
eq ?f111 +0-?f110
eq if(?f112 ,?f97 ,?f99 )
eq if(?f112 ,?f103 ,?f105 )
eq ?f113 *?f10 /?f74
eq ?f114 *?f12 /?f74
eq ?f21 +?f115 -0
eq ?f23 +?f116 -0
eq ?f70 *?f25 /?f13
eq ?f71 *?f25 /?f14
eq ?f72 *?f25 /?f13
eq ?f73 *?f25 /?f14
eq ?f121 +0-?f119
eq ?f122 +0-?f120
eq sqrt(?f123 *?f123 +?f124 *?f124 +0*0)
eq ?f119 *?f122 /1
eq ?f121 *?f120 /1
eq ?f126 +0-?f127
eq ?f25 *?f25 /1
eq ?f125 *?f125 /1
eq ?f129 *?f130 /1
eq ?f128 *?f128 /1
eq ?f131 +0-?f132
eq max(?f133 ,0)
eq sqrt(?f134 )
eq ?f93 *?f123 /1
eq ?f136 *?f135 /1
eq ?f128 *?f124 /1
eq (?f138 +?f137 )/?f130
eq ?f138 +0-?f137
eq ?f140 *1/?f130
eq abs(?f124 )
eq ?f142 *?f135 /1
eq ?f128 *?f123 /-1
eq (?f144 +?f143 )/?f130
eq ?f144 +0-?f143
eq ?f146 *1/?f130
eq ?f119 +0-?f139
eq ?f119 +0-?f141
eq ?f120 +0-?f145
eq ?f120 +0-?f147
eq sqrt(?f148 *?f148 +?f150 *?f150 +0*0)
eq sqrt(?f149 *?f149 +?f151 *?f151 +0*0)
eq ?f153 +0-?f152
eq if(?f154 ,?f139 ,?f141 )
eq if(?f154 ,?f145 ,?f147 )
eq ?f155 *?f13 /?f25
eq ?f156 *?f14 /?f25
eq ?f21 +?f157 -0
eq ?f23 +?f158 -0
eq (10800000*atan2(?f158 ,?f157 ))/pi
eq ?f161 +21600000-0
eq if(?f161 ,?f161 ,?f162 )
eq ?f4 +0-?f163
eq ?f164 +0-21600000
eq if(?f164 ,?f165 ,?f164 )
eq ?f117 +0-?f159
eq ?f118 +0-?f160
eq sqrt(?f167 *?f167 +?f168 *?f168 +0*0)
eq ?f169 *1/2
eq ?f170 +0-?f7
eq if(?f171 ,?f117 ,?f64 )
eq if(?f171 ,?f118 ,?f65 )
eq if(?f171 ,?f159 ,?f68 )
eq if(?f171 ,?f160 ,?f69 )
eq (10800000*atan2(?f116 ,?f115 ))/pi
eq ?f176 +21600000-0
eq if(?f176 ,?f176 ,?f177 )
eq ?f178 +0-?f4
eq ?f179 +21600000-0
eq if(?f179 ,?f179 ,?f180 )
eq ?f15 *sin(pi*(?f4 )/10800000)
eq ?f16 *cos(pi*(?f4 )/10800000)
eq ?f15 *(cos(atan2(?f182 ,?f183 )))
eq ?f16 *(sin(atan2(?f182 ,?f183 )))
eq ?f21 +?f184 -0
eq ?f23 +?f185 -0
eq ?f4 +0-5400000
eq ?f49 +5400000-0
eq ?f49 +10800000-0
eq ?f10 *cos(pi*(2700000)/10800000)
eq ?f12 *sin(pi*(2700000)/10800000)
eq ?f21 +0-?f191
eq ?f21 +?f191 -0
eq ?f23 +0-?f192
eq ?f23 +?f192 -0
eq (?f4 )/60000.0
eq (?f181 )/60000.0
eq (?f163 )/60000.0
eq (?f166 )/60000.0
path M ?f60 ?f61 G ?f10 ?f12 ?f197 ?f198 L ?f172 ?f173 L ?f54 ?f55 L ?f174 ?f175 L ?f159 ?f160 G ?f13 ?f14 ?f199 ?f200 Z N

[cloud]
eq logwidth*2977/21600
eq logheight*3262/21600
eq logwidth*17087/21600
eq logheight*17337/21600
eq logwidth*67/21600
eq logheight*21577/21600
eq logwidth*21582/21600
eq logheight*1235/21600
eq logheight/2
eq logwidth/2
eq (-11429249)/60000.0
eq (7426832)/60000.0
eq (-8646143)/60000.0
eq (5396714)/60000.0
eq (-8748475)/60000.0
eq (5983381)/60000.0
eq (-7859164)/60000.0
eq (7034504)/60000.0
eq (-4722533)/60000.0
eq (6541615)/60000.0
eq (-2776035)/60000.0
eq (7816140)/60000.0
eq (37501)/60000.0
eq (6842000)/60000.0
eq (1347096)/60000.0
eq (6910353)/60000.0
eq (3974558)/60000.0
eq (4542661)/60000.0
eq (-16496525)/60000.0
eq (8804134)/60000.0
eq (-14809710)/60000.0
eq (9151131)/60000.0
eq (5204520)/60000.0
eq (1585770)/60000.0
eq (4416628)/60000.0
eq (686848)/60000.0
eq (8257449)/60000.0
eq (844866)/60000.0
eq (387196)/60000.0
eq (959901)/60000.0
eq (-4217541)/60000.0
eq (4255042)/60000.0
eq (1819082)/60000.0
eq (1665090)/60000.0
eq (-824660)/60000.0
eq (891534)/60000.0
eq (-8950887)/60000.0
eq (1091722)/60000.0
eq (-9809656)/60000.0
eq (1061181)/60000.0
eq (-4002417)/60000.0
eq (739161)/60000.0
eq (9459261)/60000.0
eq (711490)/60000.0
view 43200 43200 43200 43200
path M 3900 14370 G 6753 9190 ?f10 ?f11 G 5333 7267 ?f12 ?f13 G 4365 5945 ?f14 ?f15 G 4857 6595 ?f16 ?f17 G 5333 7273 ?f18 ?f19 G 6775 9220 ?f20 ?f21 G 5785 7867 ?f22 ?f23 G 6752 9215 ?f24 ?f25 G 7720 10543 ?f26 ?f27 G 4360 5918 ?f28 ?f29 G 4345 5945 ?f30 ?f31 Z N M 4693 26177 G 4345 5945 ?f32 ?f33 M 6928 34899 G 4360 5918 ?f34 ?f35 M 16478 39090 G 6752 9215 ?f36 ?f37 M 28827 34751 G 6752 9215 ?f38 ?f39 M 34129 22954 G 5785 7867 ?f40 ?f41 M 41798 15354 G 5333 7273 ?f42 ?f43 M 38324 5426 G 4857 6595 ?f44 ?f45 M 29078 3952 G 4857 6595 ?f46 ?f47 M 22141 4720 G 4365 5945 ?f48 ?f49 M 14000 5192 G 6753 9190 ?f50 ?f51 M 4127 15789 G 6753 9190 ?f52 ?f53 F N

[cloudCallout]
mod adj1=-20833 adj2=62500
eq logwidth*$0 /100000
eq logheight*$1 /100000
eq logwidth/2
eq ?f2 +?f0 -0
eq logheight/2
eq ?f4 +?f1 -0
eq logheight/2
eq ?f6 *(cos(atan2(?f1 ,?f0 )))
eq logwidth/2
eq ?f8 *(sin(atan2(?f1 ,?f0 )))
eq ?f8 *(cos(atan2(?f9 ,?f7 )))
eq ?f6 *(sin(atan2(?f9 ,?f7 )))
eq ?f2 +?f10 -0
eq ?f4 +?f11 -0
eq ?f12 +0-?f3
eq ?f13 +0-?f5
eq sqrt(?f14 *?f14 +?f15 *?f15 +0*0)
eq min(logwidth,logheight)
eq ?f17 *6600/21600
eq ?f16 +0-?f18
eq ?f19 *1/3
eq ?f17 *1800/21600
eq ?f20 +?f21 -0
eq ?f22 *?f14 /?f16
eq ?f22 *?f15 /?f16
eq ?f23 +?f3 -0
eq ?f24 +?f5 -0
eq ?f17 *4800/21600
eq ?f20 *2/1
eq ?f27 +?f28 -0
eq ?f29 *?f14 /?f16
eq ?f29 *?f15 /?f16
eq ?f30 +?f3 -0
eq ?f31 +?f5 -0
eq ?f17 *1200/21600
eq ?f17 *600/21600
eq ?f3 +?f35 -0
eq ?f25 +?f34 -0
eq ?f32 +?f21 -0
eq logwidth*2977/21600
eq logheight*3262/21600
eq logwidth*17087/21600
eq logheight*17337/21600
eq logwidth*67/21600
eq logheight*21577/21600
eq logwidth*21582/21600
eq logheight*1235/21600
eq (10800000*atan2(?f1 ,?f0 ))/pi
eq (-11429249)/60000.0
eq (7426832)/60000.0
eq (-8646143)/60000.0
eq (5396714)/60000.0
eq (-8748475)/60000.0
eq (5983381)/60000.0
eq (-7859164)/60000.0
eq (7034504)/60000.0
eq (-4722533)/60000.0
eq (6541615)/60000.0
eq (-2776035)/60000.0
eq (7816140)/60000.0
eq (37501)/60000.0
eq (6842000)/60000.0
eq (1347096)/60000.0
eq (6910353)/60000.0
eq (3974558)/60000.0
eq (4542661)/60000.0
eq (-16496525)/60000.0
eq (8804134)/60000.0
eq (-14809710)/60000.0
eq (9151131)/60000.0
eq (0)/60000.0
eq (21600000)/60000.0
eq (0)/60000.0
eq (21600000)/60000.0
eq (0)/60000.0
eq (21600000)/60000.0
eq (5204520)/60000.0
eq (1585770)/60000.0
eq (4416628)/60000.0
eq (686848)/60000.0
eq (8257449)/60000.0
eq (844866)/60000.0
eq (387196)/60000.0
eq (959901)/60000.0
eq (-4217541)/60000.0
eq (4255042)/60000.0
eq (1819082)/60000.0
eq (1665090)/60000.0
eq (-824660)/60000.0
eq (891534)/60000.0
eq (-8950887)/60000.0
eq (1091722)/60000.0
eq (-9809656)/60000.0
eq (1061181)/60000.0
eq (-4002417)/60000.0
eq (739161)/60000.0
eq (9459261)/60000.0
eq (711490)/60000.0
view 43200 43200 0 0 0 0 0 0 43200 43200
path M 3900 14370 G 6753 9190 ?f48 ?f49 G 5333 7267 ?f50 ?f51 G 4365 5945 ?f52 ?f53 G 4857 6595 ?f54 ?f55 G 5333 7273 ?f56 ?f57 G 6775 9220 ?f58 ?f59 G 5785 7867 ?f60 ?f61 G 6752 9215 ?f62 ?f63 G 7720 10543 ?f64 ?f65 G 4360 5918 ?f66 ?f67 G 4345 5945 ?f68 ?f69 Z N M ?f36 ?f5 G ?f35 ?f35 ?f70 ?f71 Z N M ?f37 ?f26 G ?f34 ?f34 ?f72 ?f73 Z N M ?f38 ?f33 G ?f21 ?f21 ?f74 ?f75 Z N M 4693 26177 G 4345 5945 ?f76 ?f77 M 6928 34899 G 4360 5918 ?f78 ?f79 M 16478 39090 G 6752 9215 ?f80 ?f81 M 28827 34751 G 6752 9215 ?f82 ?f83 M 34129 22954 G 5785 7867 ?f84 ?f85 M 41798 15354 G 5333 7273 ?f86 ?f87 M 38324 5426 G 4857 6595 ?f88 ?f89 M 29078 3952 G 4857 6595 ?f90 ?f91 M 22141 4720 G 4365 5945 ?f92 ?f93 M 14000 5192 G 6753 9190 ?f94 ?f95 M 4127 15789 G 6753 9190 ?f96 ?f97 F N

[cornerTabs]
eq sqrt(logwidth*logwidth+logheight*logheight+0*0)
eq 1*?f0 /20
eq 0+logheight-?f1
eq 0+logwidth-?f1
eq logheight
eq logwidth
path M 0 0 L ?f1 0 L 0 ?f1 Z N M 0 ?f2 L ?f1 ?f4 L 0 ?f4 Z N M ?f3 0 L ?f5 0 L ?f5 ?f1 Z N M ?f5 ?f2 L ?f5 ?f4 L ?f3 ?f4 Z N

[cube]
mod adj=25000
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq logheight+0-?f2
eq ?f3 *1/2
eq (?f2 +logheight)/2
eq logwidth+0-?f2
eq ?f6 *1/2
eq (?f2 +logwidth)/2
eq logheight
eq logwidth
path M 0 ?f2 L ?f6 ?f2 L ?f6 ?f9 L 0 ?f9 Z S N M ?f6 ?f2 L ?f10 0 L ?f10 ?f3 L ?f6 ?f9 Z I S N M 0 ?f2 L ?f2 0 L ?f10 0 L ?f6 ?f2 Z K S N M 0 ?f2 L ?f2 0 L ?f10 0 L ?f10 ?f3 L ?f6 ?f9 L 0 ?f9 Z M 0 ?f2 L ?f6 ?f2 L ?f10 0 M ?f6 ?f2 L ?f6 ?f9 F N

[curvedConnector2]
eq logwidth
eq logheight
eq logwidth/2
eq logheight/2
path M 0 0 C ?f2 0 ?f0 ?f3 ?f0 ?f1 F N

[curvedConnector3]
mod adj1=50000
eq logwidth*$0 /100000
eq (0+?f0 )/2
eq (logwidth+?f0 )/2
eq logheight*3/4
eq logheight/2
eq logwidth
eq logheight
eq logheight/4
path M 0 0 C ?f1 0 ?f0 ?f7 ?f0 ?f4 C ?f0 ?f3 ?f2 ?f6 ?f5 ?f6 F N

[curvedConnector4]
mod adj1=50000 adj2=50000
eq logwidth*$0 /100000
eq (0+?f0 )/2
eq (logwidth+?f0 )/2
eq (?f0 +?f2 )/2
eq (?f2 +logwidth)/2
eq logheight*$1 /100000
eq (0+?f5 )/2
eq (0+?f6 )/2
eq (?f6 +?f5 )/2
eq (logheight+?f5 )/2
eq logwidth
eq logheight
path M 0 0 C ?f1 0 ?f0 ?f7 ?f0 ?f6 C ?f0 ?f8 ?f3 ?f5 ?f2 ?f5 C ?f4 ?f5 ?f10 ?f9 ?f10 ?f11 F N

[curvedConnector5]
mod adj1=50000 adj2=50000 adj3=50000
eq logwidth*$0 /100000
eq logwidth*$2 /100000
eq (?f0 +?f1 )/2
eq (0+?f0 )/2
eq (?f0 +?f2 )/2
eq (?f1 +?f2 )/2
eq (?f1 +logwidth)/2
eq logheight*$1 /100000
eq (0+?f7 )/2
eq (0+?f8 )/2
eq (?f8 +?f7 )/2
eq (logheight+?f7 )/2
eq (?f11 +?f7 )/2
eq (?f11 +logheight)/2
eq logwidth
eq logheight
path M 0 0 C ?f3 0 ?f0 ?f9 ?f0 ?f8 C ?f0 ?f10 ?f4 ?f7 ?f2 ?f7 C ?f5 ?f7 ?f1 ?f12 ?f1 ?f11 C ?f1 ?f13 ?f6 ?f15 ?f14 ?f15 F N

[curvedDownArrow]
mod adj1=25000 adj2=50000 adj3=25000
eq min(logwidth,logheight)
eq 50000*logwidth/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq ?f0 *?f3 /100000
eq ?f0 *?f2 /100000
eq (?f4 +?f5 )/4
eq logwidth/2
eq ?f7 +0-?f6
eq ?f8 *2/1
eq ?f9 *?f9 /1
eq ?f4 *?f4 /1
eq ?f10 +0-?f11
eq sqrt(?f12 )
eq ?f13 *logheight/?f9
eq 100000*?f14 /?f0
eq if(0-$2 ,0,if(?f15 -$2 ,$2 ,?f15 ))
eq ?f0 *$2 /100000
eq ?f8 +?f4 -0
eq logheight*logheight/1
eq ?f17 *?f17 /1
eq ?f19 +0-?f20
eq sqrt(?f21 )
eq ?f22 *?f8 /logheight
eq ?f8 +?f23 -0
eq ?f18 +?f23 -0
eq ?f5 +0-?f4
eq ?f26 *1/2
eq ?f24 +0-?f27
eq ?f25 +?f27 -0
eq ?f5 *1/2
eq logwidth+0-?f30
eq logheight+0-?f17
eq (10800000*atan2(?f23 ,?f17 ))/pi
eq 0+0-?f33
eq logheight+0-?f14
eq (?f8 +?f18 )/2
eq ?f4 *1/2
eq (10800000*atan2(?f37 ,?f14 ))/pi
eq 16200000+?f33 -0
eq 16200000+0-?f38
eq ?f38 +0-5400000
eq 5400000+?f38 -0
eq logheight
eq logwidth
eq logheight
eq (?f39 )/60000.0
eq (?f34 )/60000.0
eq (16200000)/60000.0
eq (?f33 )/60000.0
eq (?f40 )/60000.0
eq (?f41 )/60000.0
eq (10800000)/60000.0
eq (?f42 )/60000.0
eq (?f40 )/60000.0
eq (?f41 )/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (?f33 )/60000.0
eq (?f39 )/60000.0
eq (?f34 )/60000.0
path M ?f31 ?f43 L ?f28 ?f32 L ?f24 ?f32 G ?f8 ?f45 ?f46 ?f47 L ?f18 0 G ?f8 ?f45 ?f48 ?f49 L ?f29 ?f32 Z S N M ?f36 ?f35 G ?f8 ?f45 ?f50 ?f51 L 0 ?f43 G ?f8 ?f45 ?f52 ?f53 Z I S N M ?f36 ?f35 G ?f8 ?f45 ?f54 ?f55 L 0 ?f43 G ?f8 ?f45 ?f56 ?f57 L ?f18 0 G ?f8 ?f45 ?f58 ?f59 L ?f29 ?f32 L ?f31 ?f43 L ?f28 ?f32 L ?f24 ?f32 G ?f8 ?f45 ?f60 ?f61 F N

[curvedLeftArrow]
mod adj1=25000 adj2=50000 adj3=25000
eq min(logwidth,logheight)
eq 50000*logheight/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq if(0-$0 ,0,if(?f2 -$0 ,$0 ,?f2 ))
eq ?f0 *?f3 /100000
eq ?f0 *?f2 /100000
eq (?f4 +?f5 )/4
eq logheight/2
eq ?f7 +0-?f6
eq ?f8 *2/1
eq ?f9 *?f9 /1
eq ?f4 *?f4 /1
eq ?f10 +0-?f11
eq sqrt(?f12 )
eq ?f13 *logwidth/?f9
eq 100000*?f14 /?f0
eq if(0-$2 ,0,if(?f15 -$2 ,$2 ,?f15 ))
eq ?f0 *?f16 /100000
eq ?f8 +?f4 -0
eq logwidth*logwidth/1
eq ?f17 *?f17 /1
eq ?f19 +0-?f20
eq sqrt(?f21 )
eq ?f22 *?f8 /logwidth
eq ?f8 +?f23 -0
eq ?f18 +?f23 -0
eq ?f5 +0-?f4
eq ?f26 *1/2
eq ?f24 +0-?f27
eq ?f25 +?f27 -0
eq ?f5 *1/2
eq logheight+0-?f30
eq 0+?f17 -0
eq (10800000*atan2(?f23 ,?f17 ))/pi
eq 0+0-?f33
eq 0+?f14 -0
eq (?f8 +?f18 )/2
eq ?f4 *1/2
eq (10800000*atan2(?f37 ,?f14 ))/pi
eq ?f38 +0-?f33
eq ?f33 +?f38 -0
eq 0+0-?f38
eq logwidth
eq logheight
eq logwidth
eq (?f33 )/60000.0
eq (?f39 )/60000.0
eq (?f41 )/60000.0
eq (?f40 )/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (?f33 )/60000.0
eq (?f33 )/60000.0
eq (?f39 )/60000.0
path M 0 ?f31 L ?f32 ?f28 L ?f32 ?f24 G ?f44 ?f8 ?f45 ?f46 G ?f44 ?f8 ?f47 ?f48 L ?f32 ?f29 Z S N M ?f42 ?f18 G ?f44 ?f8 ?f49 ?f50 L 0 0 G ?f44 ?f8 ?f51 ?f52 Z I S N M ?f42 ?f18 G ?f44 ?f8 ?f53 ?f54 L 0 0 G ?f44 ?f8 ?f55 ?f56 L ?f42 ?f18 G ?f44 ?f8 ?f57 ?f58 L ?f32 ?f29 L 0 ?f31 L ?f32 ?f28 L ?f32 ?f24 G ?f44 ?f8 ?f59 ?f60 F N

[curvedRightArrow]
mod adj1=25000 adj2=50000 adj3=25000
eq min(logwidth,logheight)
eq 50000*logheight/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq if(0-$0 ,0,if(?f2 -$0 ,$0 ,?f2 ))
eq ?f0 *?f3 /100000
eq ?f0 *?f2 /100000
eq (?f4 +?f5 )/4
eq logheight/2
eq ?f7 +0-?f6
eq ?f8 *2/1
eq ?f9 *?f9 /1
eq ?f4 *?f4 /1
eq ?f10 +0-?f11
eq sqrt(?f12 )
eq ?f13 *logwidth/?f9
eq 100000*?f14 /?f0
eq if(0-$2 ,0,if(?f15 -$2 ,$2 ,?f15 ))
eq ?f0 *?f16 /100000
eq ?f8 +?f4 -0
eq logwidth*logwidth/1
eq ?f17 *?f17 /1
eq ?f19 +0-?f20
eq sqrt(?f21 )
eq ?f22 *?f8 /logwidth
eq ?f8 +?f23 -0
eq ?f18 +?f23 -0
eq ?f5 +0-?f4
eq ?f26 *1/2
eq ?f24 +0-?f27
eq ?f25 +?f27 -0
eq ?f5 *1/2
eq logheight+0-?f30
eq logwidth+0-?f17
eq (10800000*atan2(?f23 ,?f17 ))/pi
eq 10800000+0-?f33
eq 0+0-?f33
eq logwidth+0-?f14
eq (?f8 +?f18 )/2
eq ?f4 *1/2
eq (10800000*atan2(?f38 ,?f14 ))/pi
eq ?f39 +0-5400000
eq 5400000+?f39 -0
eq 10800000+0-?f39
eq logwidth
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (?f35 )/60000.0
eq (?f34 )/60000.0
eq (?f33 )/60000.0
eq (16200000)/60000.0
eq (?f40 )/60000.0
eq (?f42 )/60000.0
eq (?f41 )/60000.0
eq (10800000)/60000.0
eq (?f35 )/60000.0
eq (?f34 )/60000.0
eq (?f33 )/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (?f40 )/60000.0
path M 0 ?f8 G ?f45 ?f8 ?f46 ?f47 L ?f32 ?f28 L ?f43 ?f31 L ?f32 ?f29 L ?f32 ?f25 G ?f45 ?f8 ?f48 ?f49 Z S N M ?f43 ?f4 G ?f45 ?f8 ?f50 ?f51 G ?f45 ?f8 ?f52 ?f53 Z I S N M 0 ?f8 G ?f45 ?f8 ?f54 ?f55 L ?f32 ?f28 L ?f43 ?f31 L ?f32 ?f29 L ?f32 ?f25 G ?f45 ?f8 ?f56 ?f57 L 0 ?f8 G ?f45 ?f8 ?f58 ?f59 L ?f43 ?f4 G ?f45 ?f8 ?f60 ?f61 F N

[curvedUpArrow]
mod adj1=25000 adj2=50000 adj3=25000
eq min(logwidth,logheight)
eq 50000*logwidth/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq ?f0 *?f3 /100000
eq ?f0 *?f2 /100000
eq (?f4 +?f5 )/4
eq logwidth/2
eq ?f7 +0-?f6
eq ?f8 *2/1
eq ?f9 *?f9 /1
eq ?f4 *?f4 /1
eq ?f10 +0-?f11
eq sqrt(?f12 )
eq ?f13 *logheight/?f9
eq 100000*?f14 /?f0
eq if(0-$2 ,0,if(?f15 -$2 ,$2 ,?f15 ))
eq ?f0 *$2 /100000
eq ?f8 +?f4 -0
eq logheight*logheight/1
eq ?f17 *?f17 /1
eq ?f19 +0-?f20
eq sqrt(?f21 )
eq ?f22 *?f8 /logheight
eq ?f8 +?f23 -0
eq ?f18 +?f23 -0
eq ?f5 +0-?f4
eq ?f26 *1/2
eq ?f24 +0-?f27
eq ?f25 +?f27 -0
eq ?f5 *1/2
eq logwidth+0-?f30
eq 0+?f17 -0
eq (10800000*atan2(?f23 ,?f17 ))/pi
eq 0+0-?f33
eq 0+?f14 -0
eq (?f8 +?f18 )/2
eq ?f4 *1/2
eq (10800000*atan2(?f37 ,?f14 ))/pi
eq ?f38 +0-?f33
eq 0+0-?f39
eq 5400000+0-?f33
eq ?f33 +?f38 -0
eq 5400000+0-?f38
eq logwidth
eq logheight
eq logheight
eq (?f41 )/60000.0
eq (?f42 )/60000.0
eq (?f43 )/60000.0
eq (?f39 )/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (?f43 )/60000.0
eq (?f39 )/60000.0
eq (?f41 )/60000.0
eq (?f33 )/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
path M ?f31 0 L ?f29 ?f32 L ?f25 ?f32 G ?f8 ?f46 ?f47 ?f48 G ?f8 ?f46 ?f49 ?f50 L ?f28 ?f32 Z S N M ?f8 ?f45 G ?f8 ?f46 ?f51 ?f52 L ?f4 0 G ?f8 ?f46 ?f53 ?f54 Z I S N M ?f36 ?f35 G ?f8 ?f46 ?f55 ?f56 L ?f28 ?f32 L ?f31 0 L ?f29 ?f32 L ?f25 ?f32 G ?f8 ?f46 ?f57 ?f58 L ?f8 ?f45 G ?f8 ?f46 ?f59 ?f60 L ?f4 0 G ?f8 ?f46 ?f61 ?f62 F N

[diagStripe]
mod adj=50000
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq logwidth*?f0 /100000
eq ?f1 *1/2
eq (?f1 +logwidth)/2
eq logheight*?f0 /100000
eq ?f4 *1/2
eq (?f4 +logheight)/2
eq logwidth/2
eq logheight/2
eq logwidth
eq logheight
path M 0 ?f4 L ?f1 0 L ?f9 0 L 0 ?f10 Z N

[doubleWave]
mod adj1=6250 adj2=0
eq if(0-$0 ,0,if(12500-$0 ,$0 ,12500))
eq if(-10000-$1 ,-10000,if(10000-$1 ,$1 ,10000))
eq logheight*?f0 /100000
eq ?f2 *10/3
eq ?f2 +0-?f3
eq ?f2 +?f3 -0
eq logheight+0-?f2
eq ?f6 +0-?f3
eq ?f6 +?f3 -0
eq logwidth*?f1 /100000
eq logwidth*?f1 /50000
eq abs(?f9 )
eq if(?f10 ,0,?f10 )
eq 0+0-?f12
eq if(?f10 ,?f10 ,0)
eq logwidth+0-?f14
eq (?f12 +?f15 )/6
eq ?f13 +?f16 -0
eq (?f12 +?f15 )/3
eq ?f13 +?f18 -0
eq (?f13 +?f15 )/2
eq ?f20 +?f16 -0
eq (?f21 +?f15 )/2
eq 0+?f14 -0
eq logwidth+?f12 -0
eq ?f23 +?f16 -0
eq ?f23 +?f18 -0
eq (?f23 +?f24 )/2
eq ?f27 +?f16 -0
eq (?f28 +?f24 )/2
eq logwidth+0-?f11
eq logwidth/2
eq ?f31 +?f9 -0
eq max(?f13 ,?f23 )
eq min(?f15 ,?f24 )
eq logheight*?f0 /50000
eq logheight+0-?f35
eq logheight
eq logheight/2
path M ?f13 ?f2 C ?f17 ?f4 ?f19 ?f5 ?f20 ?f2 C ?f21 ?f4 ?f22 ?f5 ?f15 ?f2 L ?f24 ?f6 C ?f29 ?f8 ?f28 ?f7 ?f27 ?f6 C ?f26 ?f8 ?f25 ?f7 ?f23 ?f6 Z N

[downArrowCallout]
mod adj1=25000 adj2=25000 adj3=25000 adj4=64977
eq min(logwidth,logheight)
eq 50000*logwidth/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f2 *2/1
eq if(0-$0 ,0,if(?f3 -$0 ,$0 ,?f3 ))
eq 100000*logheight/?f0
eq if(0-$2 ,0,if(?f5 -$2 ,$2 ,?f5 ))
eq ?f6 *?f0 /logheight
eq 100000+0-?f7
eq if(0-$3 ,0,if(?f8 -$3 ,$3 ,?f8 ))
eq ?f0 *?f2 /100000
eq ?f0 *?f4 /200000
eq logwidth/2
eq ?f12 +0-?f10
eq ?f12 +0-?f11
eq ?f12 +?f11 -0
eq ?f12 +?f10 -0
eq ?f0 *?f6 /100000
eq logheight+0-?f17
eq logheight*?f9 /100000
eq ?f19 *1/2
eq logheight
eq logwidth
path M 0 0 L ?f22 0 L ?f22 ?f19 L ?f15 ?f19 L ?f15 ?f18 L ?f16 ?f18 L ?f12 ?f21 L ?f13 ?f18 L ?f14 ?f18 L ?f14 ?f19 L 0 ?f19 Z N

[ellipse]
eq logwidth/2
eq ?f0 *cos(pi*(2700000)/10800000)
eq logheight/2
eq ?f2 *sin(pi*(2700000)/10800000)
eq logwidth/2
eq ?f4 +0-?f1
eq ?f4 +?f1 -0
eq logheight/2
eq ?f7 +0-?f3
eq ?f7 +?f3 -0
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
path M 0 ?f7 G ?f0 ?f2 ?f12 ?f13 G ?f0 ?f2 ?f14 ?f15 G ?f0 ?f2 ?f16 ?f17 G ?f0 ?f2 ?f18 ?f19 Z N

[ellipseRibbon]
mod adj1=25000 adj2=50000 adj3=12500
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq if(25000-$1 ,25000,if(75000-$1 ,$1 ,75000))
eq 100000+0-?f0
eq ?f2 *1/2
eq ?f0 +0-?f3
eq max(0,?f4 )
eq if(?f5 -$2 ,?f5 ,if(?f0 -$2 ,$2 ,?f0 ))
eq logwidth*?f1 /200000
eq logwidth/2
eq ?f8 +0-?f7
eq logwidth/8
eq ?f9 +?f10 -0
eq logwidth+0-?f11
eq logwidth+0-?f9
eq logwidth+0-?f10
eq logheight*?f6 /100000
eq 4*?f15 /logwidth
eq ?f11 *?f11 /logwidth
eq ?f11 +0-?f17
eq ?f16 *?f18 /1
eq ?f11 *1/2
eq ?f16 *?f20 /1
eq logwidth+0-?f20
eq logheight*?f0 /100000
eq ?f23 +0-?f15
eq ?f9 *?f9 /logwidth
eq ?f9 +0-?f25
eq ?f16 *?f26 /1
eq ?f27 +?f24 -0
eq ?f15 +?f24 -?f28
eq ?f29 +?f15 -0
eq ?f30 +?f24 -0
eq logheight+0-?f23
eq ?f15 *14/16
eq (?f33 +?f32 )/2
eq ?f27 +?f32 -0
eq ?f28 +?f32 -0
eq ?f9 *1/2
eq ?f16 *?f37 /1
eq ?f38 +?f32 -0
eq logwidth+0-?f37
eq ?f31 +?f32 -0
eq ?f19 +?f24 -0
eq ?f23 +?f23 -?f42
eq logheight+0-?f15
eq logheight
eq logwidth
path M 0 0 Q ?f20 ?f21 ?f11 ?f19 L ?f9 ?f28 Q ?f8 ?f31 ?f13 ?f28 L ?f12 ?f19 Q ?f22 ?f21 ?f46 0 L ?f14 ?f34 L ?f46 ?f32 Q ?f40 ?f39 ?f13 ?f35 L ?f13 ?f36 Q ?f8 ?f41 ?f9 ?f36 L ?f9 ?f35 Q ?f37 ?f39 0 ?f32 L ?f10 ?f34 Z S N M ?f11 ?f42 L ?f11 ?f19 L ?f9 ?f28 Q ?f8 ?f31 ?f13 ?f28 L ?f12 ?f19 L ?f12 ?f42 Q ?f8 ?f43 ?f11 ?f42 Z I S N M 0 0 Q ?f20 ?f21 ?f11 ?f19 L ?f9 ?f28 Q ?f8 ?f31 ?f13 ?f28 L ?f12 ?f19 Q ?f22 ?f21 ?f46 0 L ?f14 ?f34 L ?f46 ?f32 Q ?f40 ?f39 ?f13 ?f35 L ?f13 ?f36 Q ?f8 ?f41 ?f9 ?f36 L ?f9 ?f35 Q ?f37 ?f39 0 ?f32 L ?f10 ?f34 Z M ?f9 ?f35 L ?f9 ?f28 M ?f13 ?f28 L ?f13 ?f35 M ?f11 ?f19 L ?f11 ?f42 M ?f12 ?f42 L ?f12 ?f19 F N

[ellipseRibbon2]
mod adj1=25000 adj2=50000 adj3=12500
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq if(25000-$1 ,25000,if(75000-$1 ,$1 ,75000))
eq 100000+0-?f0
eq ?f2 *1/2
eq ?f0 +0-?f3
eq max(0,?f4 )
eq if(?f5 -$2 ,?f5 ,if(?f0 -$2 ,$2 ,?f0 ))
eq logwidth*?f1 /200000
eq logwidth/2
eq ?f8 +0-?f7
eq logwidth/8
eq ?f9 +?f10 -0
eq logwidth+0-?f11
eq logwidth+0-?f9
eq logwidth+0-?f10
eq logheight*?f6 /100000
eq 4*?f15 /logwidth
eq ?f11 *?f11 /logwidth
eq ?f11 +0-?f17
eq ?f16 *?f18 /1
eq logheight+0-?f19
eq ?f11 *1/2
eq ?f16 *?f21 /1
eq logheight+0-?f22
eq logwidth+0-?f21
eq logheight*?f0 /100000
eq ?f25 +0-?f15
eq ?f9 *?f9 /logwidth
eq ?f9 +0-?f27
eq ?f16 *?f28 /1
eq ?f29 +?f26 -0
eq logheight+0-?f30
eq ?f15 +?f26 -?f30
eq ?f32 +?f15 -0
eq ?f33 +?f26 -0
eq logheight+0-?f34
eq logheight+0-?f25
eq ?f15 *14/16
eq (?f37 +?f36 )/2
eq logheight+0-?f38
eq ?f29 +?f36 -0
eq logheight+0-?f40
eq ?f30 +?f36 -0
eq logheight+0-?f42
eq ?f9 *1/2
eq ?f16 *?f44 /1
eq ?f45 +?f36 -0
eq logheight+0-?f46
eq logwidth+0-?f44
eq ?f34 +?f36 -0
eq logheight+0-?f49
eq ?f19 +?f26 -0
eq logheight+0-?f51
eq ?f25 +?f25 -?f51
eq logheight+0-?f53
eq logheight
eq logwidth
path M 0 ?f55 Q ?f21 ?f23 ?f11 ?f20 L ?f9 ?f31 Q ?f8 ?f35 ?f13 ?f31 L ?f12 ?f20 Q ?f24 ?f23 ?f56 ?f55 L ?f14 ?f39 L ?f56 ?f25 Q ?f48 ?f47 ?f13 ?f41 L ?f13 ?f43 Q ?f8 ?f50 ?f9 ?f43 L ?f9 ?f41 Q ?f44 ?f47 0 ?f25 L ?f10 ?f39 Z S N M ?f11 ?f52 L ?f11 ?f20 L ?f9 ?f31 Q ?f8 ?f35 ?f13 ?f31 L ?f12 ?f20 L ?f12 ?f52 Q ?f8 ?f54 ?f11 ?f52 Z I S N M 0 ?f55 L ?f10 ?f39 L 0 ?f25 Q ?f44 ?f47 ?f9 ?f41 L ?f9 ?f43 Q ?f8 ?f50 ?f13 ?f43 L ?f13 ?f41 Q ?f48 ?f47 ?f56 ?f25 L ?f14 ?f39 L ?f56 ?f55 Q ?f24 ?f23 ?f12 ?f20 L ?f13 ?f31 Q ?f8 ?f35 ?f9 ?f31 L ?f11 ?f20 Q ?f21 ?f23 0 ?f55 Z M ?f9 ?f31 L ?f9 ?f41 M ?f13 ?f41 L ?f13 ?f31 M ?f11 ?f52 L ?f11 ?f20 M ?f12 ?f20 L ?f12 ?f52 F N

[flowChartMagneticDisk]
eq logheight*5/6
eq logwidth/2
eq logheight/3
eq logheight/2
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
view 6 6 6 6 6 6
path M 0 1 G 3 1 ?f6 ?f7 L 6 5 G 3 1 ?f8 ?f9 Z S N M 6 1 G 3 1 ?f10 ?f11 F N M 0 1 G 3 1 ?f12 ?f13 L 6 5 G 3 1 ?f14 ?f15 Z F N

[flowChartMagneticDrum]
eq logwidth*2/3
eq logwidth/2
eq logheight/2
eq logheight
eq logwidth
eq logwidth/6
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
view 6 6 6 6 6 6
path M 1 0 L 5 0 G 1 3 ?f6 ?f7 L 1 6 G 1 3 ?f8 ?f9 Z S N M 5 6 G 1 3 ?f10 ?f11 F N M 1 0 L 5 0 G 1 3 ?f12 ?f13 L 1 6 G 1 3 ?f14 ?f15 Z F N

[flowChartMagneticTape]
eq logwidth/2
eq ?f0 *cos(pi*(2700000)/10800000)
eq logheight/2
eq ?f2 *sin(pi*(2700000)/10800000)
eq logwidth/2
eq ?f4 +0-?f1
eq ?f4 +?f1 -0
eq logheight/2
eq ?f7 +0-?f3
eq ?f7 +?f3 -0
eq (10800000*atan2(logheight,logwidth))/pi
eq logheight
eq logwidth
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (?f10 )/60000.0
path M ?f4 ?f11 G ?f0 ?f2 ?f13 ?f14 G ?f0 ?f2 ?f15 ?f16 G ?f0 ?f2 ?f17 ?f18 G ?f0 ?f2 ?f19 ?f20 L ?f12 ?f9 L ?f12 ?f11 Z N

[flowChartMultidocument]
eq logheight*3675/21600
eq logheight*20782/21600
eq logwidth*9298/21600
eq logwidth*12286/21600
eq logwidth*18595/21600
eq logheight/2
eq logwidth
view 21600 21600 21600 21600 21600 21600
path M 0 20782 C 9298 23542 9298 18022 18595 18022 L 18595 3675 L 0 3675 Z M 1532 3675 L 1532 1815 L 20000 1815 L 20000 16252 C 19298 16252 18595 16352 18595 16352 L 18595 3675 Z M 2972 1815 L 2972 0 L 21600 0 L 21600 14392 C 20800 14392 20000 14467 20000 14467 L 20000 1815 Z S N M 0 3675 L 18595 3675 L 18595 18022 C 9298 18022 9298 23542 0 20782 Z M 1532 3675 L 1532 1815 L 20000 1815 L 20000 16252 C 19298 16252 18595 16352 18595 16352 M 2972 1815 L 2972 0 L 21600 0 L 21600 14392 C 20800 14392 20000 14467 20000 14467 F N M 0 20782 C 9298 23542 9298 18022 18595 18022 L 18595 16352 C 18595 16352 19298 16252 20000 16252 L 20000 14467 C 20000 14467 20800 14392 21600 14392 L 21600 0 L 2972 0 L 2972 1815 L 1532 1815 L 1532 3675 L 0 3675 Z F S N

[flowChartOfflineStorage]
eq logwidth*3/4
eq logheight/2
eq logwidth/2
eq logheight
eq logwidth/4
view 2 2 5 5 2 2
path M 0 0 L 2 0 L 1 2 Z S N M 2 4 L 3 4 F N M 0 0 L 2 0 L 1 2 Z F N

[flowChartPunchedTape]
eq logheight*9/10
eq logheight*4/5
eq logwidth/2
eq logheight/10
eq logheight/2
eq logwidth
eq logheight/5
eq (10800000)/60000.0
eq (-10800000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (-10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
view 20 20
path M 0 2 G 5 2 ?f7 ?f8 G 5 2 ?f9 ?f10 L 20 18 G 5 2 ?f11 ?f12 G 5 2 ?f13 ?f14 Z N

[foldedCorner]
mod adj=16667
eq if(0-$0 ,0,if(50000-$0 ,$0 ,50000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq ?f2 *1/5
eq logwidth+0-?f2
eq ?f4 +?f3 -0
eq logheight+0-?f2
eq ?f6 +?f3 -0
eq logheight
eq logwidth/2
eq logheight/2
eq logwidth
path M 0 0 L ?f11 0 L ?f11 ?f6 L ?f4 ?f8 L 0 ?f8 Z S N M ?f4 ?f8 L ?f5 ?f7 L ?f11 ?f6 Z I S N M ?f4 ?f8 L ?f5 ?f7 L ?f11 ?f6 L ?f4 ?f8 L 0 ?f8 L 0 0 L ?f11 0 L ?f11 ?f6 F N

[funnel]
eq min(logwidth,logheight)
eq ?f0 *1/20
eq logwidth/2
eq ?f2 +0-?f1
eq logheight/4
eq ?f4 +0-?f1
eq ?f2 *cos(pi*(480000)/10800000)
eq ?f4 *sin(pi*(480000)/10800000)
eq (10800000*atan2(?f7 ,?f6 ))/pi
eq ?f8 *2/1
eq 10800000+0-?f8
eq 10800000+2-0
eq 10800000+0-2
eq ?f2 *1/4
eq ?f4 *1/4
eq ?f4 *cos(pi*(?f10 )/10800000)
eq ?f2 *sin(pi*(?f10 )/10800000)
eq sqrt(?f15 *?f15 +?f16 *?f16 +0*0)
eq ?f2 *?f4 /?f17
eq ?f18 *cos(pi*(?f10 )/10800000)
eq ?f18 *sin(pi*(?f10 )/10800000)
eq logwidth/2
eq ?f21 +?f19 -0
eq ?f4 +?f20 -0
eq ?f14 *cos(pi*(?f8 )/10800000)
eq ?f13 *sin(pi*(?f8 )/10800000)
eq sqrt(?f24 *?f24 +?f25 *?f25 +0*0)
eq ?f13 *?f14 /?f26
eq ?f27 *cos(pi*(?f8 )/10800000)
eq ?f27 *sin(pi*(?f8 )/10800000)
eq ?f21 +?f28 -0
eq logheight+0-?f14
eq ?f31 +?f29 -0
eq ?f2 +0-?f3
eq 10800000*2/1
eq logwidth
eq logheight
eq (?f10 )/60000.0
eq (?f11 )/60000.0
eq (?f8 )/60000.0
eq (?f12 )/60000.0
eq (10800000)/60000.0
eq (-21600000)/60000.0
path M ?f22 ?f23 G ?f2 ?f4 ?f37 ?f38 L ?f30 ?f32 G ?f13 ?f14 ?f39 ?f40 Z M ?f33 ?f4 G ?f3 ?f5 ?f41 ?f42 Z N

[gear6]
mod adj1=15000 adj2=3526
eq if(0-$0 ,0,if(20000-$0 ,$0 ,20000))
eq if(0-$1 ,0,if(5358-$1 ,$1 ,5358))
eq min(logwidth,logheight)
eq ?f2 *?f0 /100000
eq ?f2 *?f1 /100000
eq ?f3 *1/2
eq ?f4 *1/2
eq ?f5 +?f6 -0
eq logheight/2
eq ?f8 +0-?f3
eq logwidth/2
eq ?f10 +0-?f3
eq ?f11 +0-?f9
eq if(?f12 ,?f9 ,?f11 )
eq (10800000*atan2(?f7 ,?f13 ))/pi
eq 19800000+0-?f14
eq 19800000+?f14 -0
eq ?f11 *cos(pi*(?f15 )/10800000)
eq ?f9 *sin(pi*(?f15 )/10800000)
eq (10800000*atan2(?f18 ,?f17 ))/pi
eq ?f9 *cos(pi*(?f19 )/10800000)
eq ?f11 *sin(pi*(?f19 )/10800000)
eq sqrt(?f20 *?f20 +?f21 *?f21 +0*0)
eq ?f11 *?f9 /?f22
eq ?f23 *cos(pi*(?f19 )/10800000)
eq ?f23 *sin(pi*(?f19 )/10800000)
eq logwidth/2
eq ?f26 +?f24 -0
eq logheight/2
eq ?f28 +?f25 -0
eq ?f11 *cos(pi*(?f16 )/10800000)
eq ?f9 *sin(pi*(?f16 )/10800000)
eq (10800000*atan2(?f31 ,?f30 ))/pi
eq ?f9 *cos(pi*(?f32 )/10800000)
eq ?f11 *sin(pi*(?f32 )/10800000)
eq sqrt(?f33 *?f33 +?f34 *?f34 +0*0)
eq ?f11 *?f9 /?f35
eq ?f36 *cos(pi*(?f32 )/10800000)
eq ?f36 *sin(pi*(?f32 )/10800000)
eq ?f26 +?f37 -0
eq ?f28 +?f38 -0
eq ?f27 +0-?f39
eq ?f29 +0-?f40
eq sqrt(?f41 *?f41 +?f42 *?f42 +0*0)
eq (10800000*atan2(?f41 ,?f42 ))/pi
eq ?f4 *sin(pi*(?f44 )/10800000)
eq ?f4 *cos(pi*(?f44 )/10800000)
eq ?f39 +?f45 -0
eq ?f40 +?f46 -0
eq ?f27 +0-?f45
eq ?f29 +0-?f46
eq ?f3 *sin(pi*(?f44 )/10800000)
eq ?f3 *cos(pi*(?f44 )/10800000)
eq ?f48 +?f51 -0
eq ?f47 +0-?f52
eq ?f50 +?f51 -0
eq ?f49 +0-?f52
eq 16200000+?f14 -0
eq ?f11 *cos(pi*(?f57 )/10800000)
eq ?f9 *sin(pi*(?f57 )/10800000)
eq (10800000*atan2(?f59 ,?f58 ))/pi
eq ?f9 *cos(pi*(?f60 )/10800000)
eq ?f11 *sin(pi*(?f60 )/10800000)
eq sqrt(?f61 *?f61 +?f62 *?f62 +0*0)
eq ?f11 *?f9 /?f63
eq ?f64 *cos(pi*(?f60 )/10800000)
eq ?f64 *sin(pi*(?f60 )/10800000)
eq ?f26 +?f65 -0
eq ?f28 +?f66 -0
eq ?f26 +0-?f65
eq ?f67 +0-?f4
eq ?f69 +?f4 -0
eq ?f68 +0-?f3
eq ?f19 +0-?f60
eq 1800000+0-?f14
eq 1800000+?f14 -0
eq ?f11 *cos(pi*(?f74 )/10800000)
eq ?f9 *sin(pi*(?f74 )/10800000)
eq (10800000*atan2(?f77 ,?f76 ))/pi
eq logheight+0-?f40
eq ?f11 *cos(pi*(?f75 )/10800000)
eq ?f9 *sin(pi*(?f75 )/10800000)
eq (10800000*atan2(?f81 ,?f80 ))/pi
eq logheight+0-?f29
eq logheight+0-?f55
eq logheight+0-?f53
eq ?f54
eq ?f78 +0-?f32
eq 5400000+?f14 -0
eq ?f11 *cos(pi*(?f88 )/10800000)
eq ?f9 *sin(pi*(?f88 )/10800000)
eq (10800000*atan2(?f90 ,?f89 ))/pi
eq logheight+0-?f68
eq logheight+0-?f72
eq 9000000+?f14 -0
eq ?f11 *cos(pi*(?f94 )/10800000)
eq ?f9 *sin(pi*(?f94 )/10800000)
eq (10800000*atan2(?f96 ,?f95 ))/pi
eq logwidth+0-?f39
eq logwidth+0-?f54
eq logwidth+0-?f56
eq 12600000+?f14 -0
eq ?f11 *cos(pi*(?f101 )/10800000)
eq ?f9 *sin(pi*(?f101 )/10800000)
eq (10800000*atan2(?f103 ,?f102 ))/pi
eq logwidth+0-?f27
eq logwidth+0-?f56
eq logwidth+0-?f54
eq (?f56 +?f54 )/2
eq (?f55 +?f53 )/2
eq logheight+0-?f109
eq (logwidth+0)/?f108
eq (?f32 )/60000.0
eq (?f87 )/60000.0
eq (?f82 )/60000.0
eq (?f73 )/60000.0
eq (?f91 )/60000.0
eq (?f73 )/60000.0
eq (?f97 )/60000.0
eq (?f87 )/60000.0
eq (?f104 )/60000.0
eq (?f73 )/60000.0
eq (?f60 )/60000.0
eq (?f73 )/60000.0
path M ?f27 ?f29 L ?f56 ?f55 L ?f54 ?f53 L ?f39 ?f40 G ?f11 ?f9 ?f112 ?f113 L ?f54 ?f85 L ?f56 ?f84 L ?f27 ?f83 G ?f11 ?f9 ?f114 ?f115 L ?f70 ?f93 L ?f71 ?f93 L ?f69 ?f92 G ?f11 ?f9 ?f116 ?f117 L ?f100 ?f84 L ?f99 ?f85 L ?f98 ?f79 G ?f11 ?f9 ?f118 ?f119 L ?f107 ?f53 L ?f106 ?f55 L ?f105 ?f29 G ?f11 ?f9 ?f120 ?f121 L ?f71 ?f72 L ?f70 ?f72 L ?f67 ?f68 G ?f11 ?f9 ?f122 ?f123 Z N

[gear9]
mod adj1=10000 adj2=1763
eq if(0-$0 ,0,if(20000-$0 ,$0 ,20000))
eq if(0-$1 ,0,if(2679-$1 ,$1 ,2679))
eq min(logwidth,logheight)
eq ?f2 *?f0 /100000
eq ?f2 *?f1 /100000
eq ?f3 *1/2
eq ?f4 *1/2
eq ?f5 +?f6 -0
eq logheight/2
eq ?f8 +0-?f3
eq logwidth/2
eq ?f10 +0-?f3
eq ?f11 +0-?f9
eq if(?f12 ,?f9 ,?f11 )
eq (10800000*atan2(?f7 ,?f13 ))/pi
eq 18600000+0-?f14
eq 18600000+?f14 -0
eq ?f11 *cos(pi*(?f15 )/10800000)
eq ?f9 *sin(pi*(?f15 )/10800000)
eq (10800000*atan2(?f18 ,?f17 ))/pi
eq ?f9 *cos(pi*(?f19 )/10800000)
eq ?f11 *sin(pi*(?f19 )/10800000)
eq sqrt(?f20 *?f20 +?f21 *?f21 +0*0)
eq ?f11 *?f9 /?f22
eq ?f23 *cos(pi*(?f19 )/10800000)
eq ?f23 *sin(pi*(?f19 )/10800000)
eq logwidth/2
eq ?f26 +?f24 -0
eq logheight/2
eq ?f28 +?f25 -0
eq ?f11 *cos(pi*(?f16 )/10800000)
eq ?f9 *sin(pi*(?f16 )/10800000)
eq (10800000*atan2(?f31 ,?f30 ))/pi
eq ?f9 *cos(pi*(?f32 )/10800000)
eq ?f11 *sin(pi*(?f32 )/10800000)
eq sqrt(?f33 *?f33 +?f34 *?f34 +0*0)
eq ?f11 *?f9 /?f35
eq ?f36 *cos(pi*(?f32 )/10800000)
eq ?f36 *sin(pi*(?f32 )/10800000)
eq ?f26 +?f37 -0
eq ?f28 +?f38 -0
eq ?f27 +0-?f39
eq ?f29 +0-?f40
eq sqrt(?f41 *?f41 +?f42 *?f42 +0*0)
eq (10800000*atan2(?f41 ,?f42 ))/pi
eq ?f4 *sin(pi*(?f44 )/10800000)
eq ?f4 *cos(pi*(?f44 )/10800000)
eq ?f39 +?f45 -0
eq ?f40 +?f46 -0
eq ?f27 +0-?f45
eq ?f29 +0-?f46
eq ?f3 *sin(pi*(?f44 )/10800000)
eq ?f3 *cos(pi*(?f44 )/10800000)
eq ?f48 +?f51 -0
eq ?f47 +0-?f52
eq ?f50 +?f51 -0
eq ?f49 +0-?f52
eq 21000000+0-?f14
eq 21000000+?f14 -0
eq ?f11 *cos(pi*(?f57 )/10800000)
eq ?f9 *sin(pi*(?f57 )/10800000)
eq (10800000*atan2(?f60 ,?f59 ))/pi
eq ?f9 *cos(pi*(?f61 )/10800000)
eq ?f11 *sin(pi*(?f61 )/10800000)
eq sqrt(?f62 *?f62 +?f63 *?f63 +0*0)
eq ?f11 *?f9 /?f64
eq ?f65 *cos(pi*(?f61 )/10800000)
eq ?f65 *sin(pi*(?f61 )/10800000)
eq ?f26 +?f66 -0
eq ?f28 +?f67 -0
eq ?f11 *cos(pi*(?f58 )/10800000)
eq ?f9 *sin(pi*(?f58 )/10800000)
eq (10800000*atan2(?f71 ,?f70 ))/pi
eq ?f9 *cos(pi*(?f72 )/10800000)
eq ?f11 *sin(pi*(?f72 )/10800000)
eq sqrt(?f73 *?f73 +?f74 *?f74 +0*0)
eq ?f11 *?f9 /?f75
eq ?f76 *cos(pi*(?f72 )/10800000)
eq ?f76 *sin(pi*(?f72 )/10800000)
eq ?f26 +?f77 -0
eq ?f28 +?f78 -0
eq ?f68 +0-?f79
eq ?f69 +0-?f80
eq sqrt(?f81 *?f81 +?f82 *?f82 +0*0)
eq (10800000*atan2(?f81 ,?f82 ))/pi
eq ?f4 *sin(pi*(?f84 )/10800000)
eq ?f4 *cos(pi*(?f84 )/10800000)
eq ?f79 +?f85 -0
eq ?f80 +?f86 -0
eq ?f68 +0-?f85
eq ?f69 +0-?f86
eq ?f3 *sin(pi*(?f84 )/10800000)
eq ?f3 *cos(pi*(?f84 )/10800000)
eq ?f88 +?f91 -0
eq ?f87 +0-?f92
eq ?f90 +?f91 -0
eq ?f89 +0-?f92
eq ?f61 +0-?f32
eq 1800000+0-?f14
eq 1800000+?f14 -0
eq ?f11 *cos(pi*(?f98 )/10800000)
eq ?f9 *sin(pi*(?f98 )/10800000)
eq (10800000*atan2(?f101 ,?f100 ))/pi
eq ?f9 *cos(pi*(?f102 )/10800000)
eq ?f11 *sin(pi*(?f102 )/10800000)
eq sqrt(?f103 *?f103 +?f104 *?f104 +0*0)
eq ?f11 *?f9 /?f105
eq ?f106 *cos(pi*(?f102 )/10800000)
eq ?f106 *sin(pi*(?f102 )/10800000)
eq ?f26 +?f107 -0
eq ?f28 +?f108 -0
eq ?f11 *cos(pi*(?f99 )/10800000)
eq ?f9 *sin(pi*(?f99 )/10800000)
eq (10800000*atan2(?f112 ,?f111 ))/pi
eq ?f9 *cos(pi*(?f113 )/10800000)
eq ?f11 *sin(pi*(?f113 )/10800000)
eq sqrt(?f114 *?f114 +?f115 *?f115 +0*0)
eq ?f11 *?f9 /?f116
eq ?f117 *cos(pi*(?f113 )/10800000)
eq ?f117 *sin(pi*(?f113 )/10800000)
eq ?f26 +?f118 -0
eq ?f28 +?f119 -0
eq ?f109 +0-?f120
eq ?f110 +0-?f121
eq sqrt(?f122 *?f122 +?f123 *?f123 +0*0)
eq (10800000*atan2(?f122 ,?f123 ))/pi
eq ?f4 *sin(pi*(?f125 )/10800000)
eq ?f4 *cos(pi*(?f125 )/10800000)
eq ?f120 +?f126 -0
eq ?f121 +?f127 -0
eq ?f109 +0-?f126
eq ?f110 +0-?f127
eq ?f3 *sin(pi*(?f125 )/10800000)
eq ?f3 *cos(pi*(?f125 )/10800000)
eq ?f129 +?f132 -0
eq ?f128 +0-?f133
eq ?f131 +?f132 -0
eq ?f130 +0-?f133
eq ?f102 +0-?f72
eq 4200000+0-?f14
eq 4200000+?f14 -0
eq ?f11 *cos(pi*(?f139 )/10800000)
eq ?f9 *sin(pi*(?f139 )/10800000)
eq (10800000*atan2(?f142 ,?f141 ))/pi
eq ?f9 *cos(pi*(?f143 )/10800000)
eq ?f11 *sin(pi*(?f143 )/10800000)
eq sqrt(?f144 *?f144 +?f145 *?f145 +0*0)
eq ?f11 *?f9 /?f146
eq ?f147 *cos(pi*(?f143 )/10800000)
eq ?f147 *sin(pi*(?f143 )/10800000)
eq ?f26 +?f148 -0
eq ?f28 +?f149 -0
eq ?f11 *cos(pi*(?f140 )/10800000)
eq ?f9 *sin(pi*(?f140 )/10800000)
eq (10800000*atan2(?f153 ,?f152 ))/pi
eq ?f9 *cos(pi*(?f154 )/10800000)
eq ?f11 *sin(pi*(?f154 )/10800000)
eq sqrt(?f155 *?f155 +?f156 *?f156 +0*0)
eq ?f11 *?f9 /?f157
eq ?f158 *cos(pi*(?f154 )/10800000)
eq ?f158 *sin(pi*(?f154 )/10800000)
eq ?f26 +?f159 -0
eq ?f28 +?f160 -0
eq ?f150 +0-?f161
eq ?f151 +0-?f162
eq sqrt(?f163 *?f163 +?f164 *?f164 +0*0)
eq (10800000*atan2(?f163 ,?f164 ))/pi
eq ?f4 *sin(pi*(?f166 )/10800000)
eq ?f4 *cos(pi*(?f166 )/10800000)
eq ?f161 +?f167 -0
eq ?f162 +?f168 -0
eq ?f150 +0-?f167
eq ?f151 +0-?f168
eq ?f3 *sin(pi*(?f166 )/10800000)
eq ?f3 *cos(pi*(?f166 )/10800000)
eq ?f170 +?f173 -0
eq ?f169 +0-?f174
eq ?f172 +?f173 -0
eq ?f171 +0-?f174
eq ?f143 +0-?f113
eq 6600000+0-?f14
eq 6600000+?f14 -0
eq ?f11 *cos(pi*(?f180 )/10800000)
eq ?f9 *sin(pi*(?f180 )/10800000)
eq (10800000*atan2(?f183 ,?f182 ))/pi
eq ?f11 *cos(pi*(?f181 )/10800000)
eq ?f9 *sin(pi*(?f181 )/10800000)
eq (10800000*atan2(?f186 ,?f185 ))/pi
eq logwidth+0-?f150
eq logwidth+0-?f178
eq logwidth+0-?f176
eq ?f184 +0-?f154
eq 9000000+?f14 -0
eq ?f11 *cos(pi*(?f192 )/10800000)
eq ?f9 *sin(pi*(?f192 )/10800000)
eq (10800000*atan2(?f194 ,?f193 ))/pi
eq logwidth+0-?f109
eq logwidth+0-?f137
eq logwidth+0-?f135
eq 11400000+?f14 -0
eq ?f11 *cos(pi*(?f199 )/10800000)
eq ?f9 *sin(pi*(?f199 )/10800000)
eq (10800000*atan2(?f201 ,?f200 ))/pi
eq logwidth+0-?f68
eq logwidth+0-?f96
eq logwidth+0-?f94
eq 13800000+?f14 -0
eq ?f11 *cos(pi*(?f206 )/10800000)
eq ?f9 *sin(pi*(?f206 )/10800000)
eq (10800000*atan2(?f208 ,?f207 ))/pi
eq logwidth+0-?f39
eq logwidth+0-?f27
eq logwidth+0-?f56
eq logwidth+0-?f54
eq 16200000+0-?f14
eq 16200000+?f14 -0
eq ?f11 *cos(pi*(?f215 )/10800000)
eq ?f9 *sin(pi*(?f215 )/10800000)
eq (10800000*atan2(?f217 ,?f216 ))/pi
eq ?f9 *cos(pi*(?f218 )/10800000)
eq ?f11 *sin(pi*(?f218 )/10800000)
eq sqrt(?f219 *?f219 +?f220 *?f220 +0*0)
eq ?f11 *?f9 /?f221
eq ?f222 *cos(pi*(?f218 )/10800000)
eq ?f222 *sin(pi*(?f218 )/10800000)
eq ?f26 +?f223 -0
eq ?f28 +?f224 -0
eq ?f11 *cos(pi*(?f214 )/10800000)
eq ?f9 *sin(pi*(?f214 )/10800000)
eq (10800000*atan2(?f228 ,?f227 ))/pi
eq ?f26 +0-?f223
eq ?f225 +0-?f4
eq ?f230 +?f4 -0
eq ?f226 +0-?f3
eq ?f229 +0-?f209
eq (?f56 +?f54 )/2
eq (?f55 +?f53 )/2
eq (?f96 +?f94 )/2
eq (?f95 +?f93 )/2
eq (?f137 +?f135 )/2
eq (?f136 +?f134 )/2
eq (?f178 +?f176 )/2
eq (?f177 +?f175 )/2
eq (logwidth+0)/?f241
eq (logwidth+0)/?f239
eq (logwidth+0)/?f237
eq (logwidth+0)/?f235
eq (?f32 )/60000.0
eq (?f97 )/60000.0
eq (?f72 )/60000.0
eq (?f138 )/60000.0
eq (?f113 )/60000.0
eq (?f179 )/60000.0
eq (?f154 )/60000.0
eq (?f191 )/60000.0
eq (?f187 )/60000.0
eq (?f179 )/60000.0
eq (?f195 )/60000.0
eq (?f138 )/60000.0
eq (?f202 )/60000.0
eq (?f97 )/60000.0
eq (?f209 )/60000.0
eq (?f234 )/60000.0
eq (?f218 )/60000.0
eq (?f234 )/60000.0
path M ?f27 ?f29 L ?f56 ?f55 L ?f54 ?f53 L ?f39 ?f40 G ?f11 ?f9 ?f247 ?f248 L ?f96 ?f95 L ?f94 ?f93 L ?f79 ?f80 G ?f11 ?f9 ?f249 ?f250 L ?f137 ?f136 L ?f135 ?f134 L ?f120 ?f121 G ?f11 ?f9 ?f251 ?f252 L ?f178 ?f177 L ?f176 ?f175 L ?f161 ?f162 G ?f11 ?f9 ?f253 ?f254 L ?f190 ?f175 L ?f189 ?f177 L ?f188 ?f151 G ?f11 ?f9 ?f255 ?f256 L ?f198 ?f134 L ?f197 ?f136 L ?f196 ?f110 G ?f11 ?f9 ?f257 ?f258 L ?f205 ?f93 L ?f204 ?f95 L ?f203 ?f69 G ?f11 ?f9 ?f259 ?f260 L ?f213 ?f53 L ?f212 ?f55 L ?f211 ?f29 G ?f11 ?f9 ?f261 ?f262 L ?f232 ?f233 L ?f231 ?f233 L ?f225 ?f226 G ?f11 ?f9 ?f263 ?f264 Z N

[horizontalScroll]
mod adj=12500
eq if(0-$0 ,0,if(25000-$0 ,$0 ,25000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq ?f2 *1/2
eq ?f2 *1/4
eq ?f2 +?f3 -0
eq ?f2 +?f2 -0
eq logheight+0-?f2
eq logheight+0-?f3
eq ?f7 +0-?f3
eq logwidth+0-?f2
eq logwidth+0-?f3
eq logwidth/2
eq logheight/2
eq logwidth
eq (0)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (-16200000)/60000.0
eq (10800000)/60000.0
eq (-10800000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (10800000)/60000.0
path M ?f14 ?f3 G ?f3 ?f3 ?f15 ?f16 L ?f11 ?f3 G ?f4 ?f4 ?f17 ?f18 L ?f10 ?f2 L ?f3 ?f2 G ?f3 ?f3 ?f19 ?f20 L 0 ?f8 G ?f3 ?f3 ?f21 ?f22 L ?f2 ?f7 L ?f11 ?f7 G ?f3 ?f3 ?f23 ?f24 Z M ?f3 ?f6 G ?f3 ?f3 ?f25 ?f26 G ?f4 ?f4 ?f27 ?f28 Z S N M ?f3 ?f6 G ?f3 ?f3 ?f29 ?f30 G ?f4 ?f4 ?f31 ?f32 Z M ?f11 ?f2 G ?f3 ?f3 ?f33 ?f34 G ?f4 ?f4 ?f35 ?f36 Z I S N M 0 ?f5 G ?f3 ?f3 ?f37 ?f38 L ?f10 ?f2 L ?f10 ?f3 G ?f3 ?f3 ?f39 ?f40 L ?f14 ?f9 G ?f3 ?f3 ?f41 ?f42 L ?f2 ?f7 L ?f2 ?f8 G ?f3 ?f3 ?f43 ?f44 Z M ?f10 ?f2 L ?f11 ?f2 G ?f3 ?f3 ?f45 ?f46 M ?f11 ?f2 L ?f11 ?f3 G ?f4 ?f4 ?f47 ?f48 M ?f3 ?f6 L ?f3 ?f5 G ?f4 ?f4 ?f49 ?f50 G ?f3 ?f3 ?f51 ?f52 M ?f2 ?f5 L ?f2 ?f7 F N

[leftArrowCallout]
mod adj1=25000 adj2=25000 adj3=25000 adj4=64977
eq min(logwidth,logheight)
eq 50000*logheight/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f2 *2/1
eq if(0-$0 ,0,if(?f3 -$0 ,$0 ,?f3 ))
eq 100000*logwidth/?f0
eq if(0-$2 ,0,if(?f5 -$2 ,$2 ,?f5 ))
eq ?f6 *?f0 /logwidth
eq 100000+0-?f7
eq if(0-$3 ,0,if(?f8 -$3 ,$3 ,?f8 ))
eq ?f0 *?f2 /100000
eq ?f0 *?f4 /200000
eq logheight/2
eq ?f12 +0-?f10
eq ?f12 +0-?f11
eq ?f12 +?f11 -0
eq ?f12 +?f10 -0
eq ?f0 *?f6 /100000
eq logwidth*?f9 /100000
eq logwidth+0-?f18
eq (?f19 +logwidth)/2
eq logheight
eq logwidth
path M 0 ?f12 L ?f17 ?f13 L ?f17 ?f14 L ?f19 ?f14 L ?f19 0 L ?f22 0 L ?f22 ?f21 L ?f19 ?f21 L ?f19 ?f15 L ?f17 ?f15 L ?f17 ?f16 Z N

[leftBrace]
mod adj1=8333 adj2=50000
eq if(0-$1 ,0,if(100000-$1 ,$1 ,100000))
eq 100000+0-?f0
eq min(?f1 ,?f0 )
eq ?f2 *1/2
eq min(logwidth,logheight)
eq ?f3 *logheight/?f4
eq if(0-$0 ,0,if(?f5 -$0 ,$0 ,?f5 ))
eq ?f4 *?f6 /100000
eq logheight*?f0 /100000
eq ?f8 +?f7 -0
eq logwidth/2
eq ?f10 *cos(pi*(2700000)/10800000)
eq ?f7 *sin(pi*(2700000)/10800000)
eq logwidth+0-?f11
eq ?f7 +0-?f12
eq logheight+?f12 -?f7
eq logwidth/2
eq logwidth
eq logheight
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
path M ?f17 ?f18 G ?f10 ?f7 ?f19 ?f20 L ?f16 ?f9 G ?f10 ?f7 ?f21 ?f22 G ?f10 ?f7 ?f23 ?f24 L ?f16 ?f7 G ?f10 ?f7 ?f25 ?f26 Z S N M ?f17 ?f18 G ?f10 ?f7 ?f27 ?f28 L ?f16 ?f9 G ?f10 ?f7 ?f29 ?f30 G ?f10 ?f7 ?f31 ?f32 L ?f16 ?f7 G ?f10 ?f7 ?f33 ?f34 F N

[leftCircularArrow]
mod adj1=12500 adj2=-1142319 adj3=1142319 adj4=10800000 adj5=12500
eq if(0-$4 ,0,if(25000-$4 ,$4 ,25000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq if(1-$2 ,1,if(21599999-$2 ,$2 ,21599999))
eq if(0-$3 ,0,if(21599999-$3 ,$3 ,21599999))
eq min(logwidth,logheight)
eq ?f5 *?f2 /100000
eq ?f5 *?f0 /100000
eq ?f6 *1/2
eq logwidth/2
eq ?f9 +?f8 -?f7
eq logheight/2
eq ?f11 +?f8 -?f7
eq ?f10 +0-?f6
eq ?f12 +0-?f6
eq ?f13 +?f8 -0
eq ?f14 +?f8 -0
eq ?f15 *sin(pi*(?f3 )/10800000)
eq ?f16 *cos(pi*(?f3 )/10800000)
eq ?f15 *(cos(atan2(?f17 ,?f18 )))
eq ?f16 *(sin(atan2(?f17 ,?f18 )))
eq logwidth/2
eq ?f21 +?f19 -0
eq logheight/2
eq ?f23 +?f20 -0
eq min(?f13 ,?f14 )
eq ?f19 *?f19 /1
eq ?f20 *?f20 /1
eq ?f25 *?f25 /1
eq ?f26 +0-?f28
eq ?f27 +0-?f28
eq ?f29 *?f30 /?f26
eq ?f31 *1/?f27
eq 1+0-?f32
eq sqrt(?f33 )
eq ?f29 *1/?f19
eq ?f35 *1/?f20
eq (1+?f34 )/?f36
eq (10800000*atan2(?f37 ,1))/pi
eq ?f38 +21600000-0
eq if(?f38 ,?f38 ,?f39 )
eq ?f40 +0-?f3
eq ?f41 +21600000-0
eq if(?f41 ,?f41 ,?f42 )
eq ?f43 +0-10800000
eq ?f43 +0-21600000
eq if(?f44 ,?f45 ,?f43 )
eq abs(?f46 )
eq ?f47 *-1/1
eq abs($1 )
eq ?f49 *-1/1
eq if(?f48 -?f50 ,?f48 ,if(0-?f50 ,?f50 ,0))
eq ?f3 +?f51 -0
eq ?f15 *sin(pi*(?f52 )/10800000)
eq ?f16 *cos(pi*(?f52 )/10800000)
eq ?f15 *(cos(atan2(?f53 ,?f54 )))
eq ?f16 *(sin(atan2(?f53 ,?f54 )))
eq ?f21 +?f55 -0
eq ?f23 +?f56 -0
eq ?f10 *sin(pi*(?f4 )/10800000)
eq ?f12 *cos(pi*(?f4 )/10800000)
eq ?f10 *(cos(atan2(?f59 ,?f60 )))
eq ?f12 *(sin(atan2(?f59 ,?f60 )))
eq ?f21 +?f61 -0
eq ?f23 +?f62 -0
eq ?f13 *sin(pi*(?f4 )/10800000)
eq ?f14 *cos(pi*(?f4 )/10800000)
eq ?f13 *(cos(atan2(?f65 ,?f66 )))
eq ?f14 *(sin(atan2(?f65 ,?f66 )))
eq ?f21 +?f67 -0
eq ?f23 +?f68 -0
eq ?f7 *cos(pi*(?f52 )/10800000)
eq ?f7 *sin(pi*(?f52 )/10800000)
eq ?f22 +?f71 -0
eq ?f24 +?f72 -0
eq ?f7 *cos(pi*(?f52 )/10800000)
eq ?f7 *sin(pi*(?f52 )/10800000)
eq ?f22 +0-?f75
eq ?f24 +0-?f76
eq ?f77 +0-?f21
eq ?f78 +0-?f23
eq ?f73 +0-?f21
eq ?f74 +0-?f23
eq min(?f10 ,?f12 )
eq ?f79 *?f83 /?f10
eq ?f80 *?f83 /?f12
eq ?f81 *?f83 /?f10
eq ?f82 *?f83 /?f12
eq ?f86 +0-?f84
eq ?f87 +0-?f85
eq sqrt(?f88 *?f88 +?f89 *?f89 +0*0)
eq ?f84 *?f87 /1
eq ?f86 *?f85 /1
eq ?f91 +0-?f92
eq ?f83 *?f83 /1
eq ?f90 *?f90 /1
eq ?f94 *?f95 /1
eq ?f93 *?f93 /1
eq ?f96 +0-?f97
eq max(?f98 ,0)
eq sqrt(?f99 )
eq ?f89 *-1/1
eq if(?f101 ,-1,1)
eq ?f102 *?f88 /1
eq ?f103 *?f100 /1
eq ?f93 *?f89 /1
eq (?f105 +?f104 )/?f95
eq ?f105 +0-?f104
eq ?f107 *1/?f95
eq abs(?f89 )
eq ?f109 *?f100 /1
eq ?f93 *?f88 /-1
eq (?f111 +?f110 )/?f95
eq ?f111 +0-?f110
eq ?f113 *1/?f95
eq ?f86 +0-?f106
eq ?f86 +0-?f108
eq ?f87 +0-?f112
eq ?f87 +0-?f114
eq sqrt(?f115 *?f115 +?f117 *?f117 +0*0)
eq sqrt(?f116 *?f116 +?f118 *?f118 +0*0)
eq ?f120 +0-?f119
eq if(?f121 ,?f106 ,?f108 )
eq if(?f121 ,?f112 ,?f114 )
eq ?f122 *?f10 /?f83
eq ?f123 *?f12 /?f83
eq ?f21 +?f124 -0
eq ?f23 +?f125 -0
eq ?f79 *?f25 /?f13
eq ?f80 *?f25 /?f14
eq ?f81 *?f25 /?f13
eq ?f82 *?f25 /?f14
eq ?f130 +0-?f128
eq ?f131 +0-?f129
eq sqrt(?f132 *?f132 +?f133 *?f133 +0*0)
eq ?f128 *?f131 /1
eq ?f130 *?f129 /1
eq ?f135 +0-?f136
eq ?f25 *?f25 /1
eq ?f134 *?f134 /1
eq ?f138 *?f139 /1
eq ?f137 *?f137 /1
eq ?f140 +0-?f141
eq max(?f142 ,0)
eq sqrt(?f143 )
eq ?f102 *?f132 /1
eq ?f145 *?f144 /1
eq ?f137 *?f133 /1
eq (?f147 +?f146 )/?f139
eq ?f147 +0-?f146
eq ?f149 *1/?f139
eq abs(?f133 )
eq ?f151 *?f144 /1
eq ?f137 *?f132 /-1
eq (?f153 +?f152 )/?f139
eq ?f153 +0-?f152
eq ?f155 *1/?f139
eq ?f128 +0-?f148
eq ?f128 +0-?f150
eq ?f129 +0-?f154
eq ?f129 +0-?f156
eq sqrt(?f157 *?f157 +?f159 *?f159 +0*0)
eq sqrt(?f158 *?f158 +?f160 *?f160 +0*0)
eq ?f162 +0-?f161
eq if(?f163 ,?f148 ,?f150 )
eq if(?f163 ,?f154 ,?f156 )
eq ?f164 *?f13 /?f25
eq ?f165 *?f14 /?f25
eq ?f21 +?f166 -0
eq ?f23 +?f167 -0
eq (10800000*atan2(?f167 ,?f166 ))/pi
eq ?f170 +21600000-0
eq if(?f170 ,?f170 ,?f171 )
eq ?f4 +0-?f172
eq ?f173 +21600000-0
eq if(?f173 ,?f173 ,?f174 )
eq ?f172 +?f175 -0
eq 0+0-?f175
eq ?f126 +0-?f168
eq ?f127 +0-?f169
eq sqrt(?f178 *?f178 +?f179 *?f179 +0*0)
eq ?f180 *1/2
eq ?f181 +0-?f7
eq if(?f182 ,?f126 ,?f73 )
eq if(?f182 ,?f127 ,?f74 )
eq if(?f182 ,?f168 ,?f77 )
eq if(?f182 ,?f169 ,?f78 )
eq (10800000*atan2(?f125 ,?f124 ))/pi
eq ?f187 +21600000-0
eq if(?f187 ,?f187 ,?f188 )
eq ?f189 +0-?f4
eq ?f190 +0-21600000
eq if(?f190 ,?f191 ,?f190 )
eq ?f4 +?f192 -0
eq 0+0-?f192
eq ?f15 *sin(pi*(?f4 )/10800000)
eq ?f16 *cos(pi*(?f4 )/10800000)
eq ?f15 *(cos(atan2(?f195 ,?f196 )))
eq ?f16 *(sin(atan2(?f195 ,?f196 )))
eq ?f21 +?f197 -0
eq ?f23 +?f198 -0
eq ?f4 +5400000-0
eq ?f52 +0-5400000
eq ?f52 +10800000-0
eq ?f10 *cos(pi*(2700000)/10800000)
eq ?f12 *sin(pi*(2700000)/10800000)
eq ?f21 +0-?f204
eq ?f21 +?f204 -0
eq ?f23 +0-?f205
eq ?f23 +?f205 -0
eq (?f176 )/60000.0
eq (?f177 )/60000.0
eq (?f193 )/60000.0
eq (?f194 )/60000.0
path M ?f63 ?f64 L ?f69 ?f70 G ?f13 ?f14 ?f210 ?f211 L ?f185 ?f186 L ?f57 ?f58 L ?f183 ?f184 L ?f126 ?f127 G ?f10 ?f12 ?f212 ?f213 Z N

[leftRightArrowCallout]
mod adj1=25000 adj2=25000 adj3=25000 adj4=48123
eq min(logwidth,logheight)
eq 50000*logheight/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f2 *2/1
eq if(0-$0 ,0,if(?f3 -$0 ,$0 ,?f3 ))
eq 50000*logwidth/?f0
eq if(0-$2 ,0,if(?f5 -$2 ,$2 ,?f5 ))
eq logwidth/2
eq ?f6 *?f0 /?f7
eq 100000+0-?f8
eq if(0-$3 ,0,if(?f9 -$3 ,$3 ,?f9 ))
eq ?f0 *?f2 /100000
eq ?f0 *?f4 /200000
eq logheight/2
eq ?f13 +0-?f11
eq ?f13 +0-?f12
eq ?f13 +?f12 -0
eq ?f13 +?f11 -0
eq ?f0 *?f6 /100000
eq logwidth+0-?f18
eq logwidth*?f10 /200000
eq logwidth/2
eq ?f21 +0-?f20
eq ?f21 +?f20 -0
eq logheight
eq logwidth
path M 0 ?f13 L ?f18 ?f14 L ?f18 ?f15 L ?f22 ?f15 L ?f22 0 L ?f23 0 L ?f23 ?f15 L ?f19 ?f15 L ?f19 ?f14 L ?f25 ?f13 L ?f19 ?f17 L ?f19 ?f16 L ?f23 ?f16 L ?f23 ?f24 L ?f22 ?f24 L ?f22 ?f16 L ?f18 ?f16 L ?f18 ?f17 Z N

[leftRightCircularArrow]
mod adj1=12500 adj2=1142319 adj3=20457681 adj4=11942319 adj5=12500
eq if(0-$4 ,0,if(25000-$4 ,$4 ,25000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq if(1-$2 ,1,if(21599999-$2 ,$2 ,21599999))
eq if(0-$3 ,0,if(21599999-$3 ,$3 ,21599999))
eq min(logwidth,logheight)
eq ?f5 *?f2 /100000
eq ?f5 *?f0 /100000
eq ?f6 *1/2
eq logwidth/2
eq ?f9 +?f8 -?f7
eq logheight/2
eq ?f11 +?f8 -?f7
eq ?f10 +0-?f6
eq ?f12 +0-?f6
eq ?f13 +?f8 -0
eq ?f14 +?f8 -0
eq ?f15 *sin(pi*(?f3 )/10800000)
eq ?f16 *cos(pi*(?f3 )/10800000)
eq ?f15 *(cos(atan2(?f17 ,?f18 )))
eq ?f16 *(sin(atan2(?f17 ,?f18 )))
eq logwidth/2
eq ?f21 +?f19 -0
eq logheight/2
eq ?f23 +?f20 -0
eq min(?f13 ,?f14 )
eq ?f19 *?f19 /1
eq ?f20 *?f20 /1
eq ?f25 *?f25 /1
eq ?f26 +0-?f28
eq ?f27 +0-?f28
eq ?f29 *?f30 /?f26
eq ?f31 *1/?f27
eq 1+0-?f32
eq sqrt(?f33 )
eq ?f29 *1/?f19
eq ?f35 *1/?f20
eq (1+?f34 )/?f36
eq (10800000*atan2(?f37 ,1))/pi
eq ?f38 +21600000-0
eq if(?f38 ,?f38 ,?f39 )
eq ?f40 +0-?f3
eq ?f41 +21600000-0
eq if(?f41 ,?f41 ,?f42 )
eq ?f43 +0-10800000
eq ?f43 +0-21600000
eq if(?f44 ,?f45 ,?f43 )
eq abs(?f46 )
eq if(0-$1 ,0,if(?f47 -$1 ,$1 ,?f47 ))
eq ?f3 +?f48 -0
eq ?f15 *sin(pi*(?f49 )/10800000)
eq ?f16 *cos(pi*(?f49 )/10800000)
eq ?f15 *(cos(atan2(?f50 ,?f51 )))
eq ?f16 *(sin(atan2(?f50 ,?f51 )))
eq ?f21 +?f52 -0
eq ?f23 +?f53 -0
eq ?f7 *cos(pi*(?f49 )/10800000)
eq ?f7 *sin(pi*(?f49 )/10800000)
eq ?f22 +?f56 -0
eq ?f24 +?f57 -0
eq ?f7 *cos(pi*(?f49 )/10800000)
eq ?f7 *sin(pi*(?f49 )/10800000)
eq ?f22 +0-?f60
eq ?f24 +0-?f61
eq ?f62 +0-?f21
eq ?f63 +0-?f23
eq ?f58 +0-?f21
eq ?f59 +0-?f23
eq min(?f10 ,?f12 )
eq ?f64 *?f68 /?f10
eq ?f65 *?f68 /?f12
eq ?f66 *?f68 /?f10
eq ?f67 *?f68 /?f12
eq ?f71 +0-?f69
eq ?f72 +0-?f70
eq sqrt(?f73 *?f73 +?f74 *?f74 +0*0)
eq ?f69 *?f72 /1
eq ?f71 *?f70 /1
eq ?f76 +0-?f77
eq ?f68 *?f68 /1
eq ?f75 *?f75 /1
eq ?f79 *?f80 /1
eq ?f78 *?f78 /1
eq ?f81 +0-?f82
eq max(?f83 ,0)
eq sqrt(?f84 )
eq ?f74 *-1/1
eq if(?f86 ,-1,1)
eq ?f87 *?f73 /1
eq ?f88 *?f85 /1
eq ?f78 *?f74 /1
eq (?f90 +?f89 )/?f80
eq ?f90 +0-?f89
eq ?f92 *1/?f80
eq abs(?f74 )
eq ?f94 *?f85 /1
eq ?f78 *?f73 /-1
eq (?f96 +?f95 )/?f80
eq ?f96 +0-?f95
eq ?f98 *1/?f80
eq ?f71 +0-?f91
eq ?f71 +0-?f93
eq ?f72 +0-?f97
eq ?f72 +0-?f99
eq sqrt(?f100 *?f100 +?f102 *?f102 +0*0)
eq sqrt(?f101 *?f101 +?f103 *?f103 +0*0)
eq ?f105 +0-?f104
eq if(?f106 ,?f91 ,?f93 )
eq if(?f106 ,?f97 ,?f99 )
eq ?f107 *?f10 /?f68
eq ?f108 *?f12 /?f68
eq ?f21 +?f109 -0
eq ?f23 +?f110 -0
eq ?f64 *?f25 /?f13
eq ?f65 *?f25 /?f14
eq ?f66 *?f25 /?f13
eq ?f67 *?f25 /?f14
eq ?f115 +0-?f113
eq ?f116 +0-?f114
eq sqrt(?f117 *?f117 +?f118 *?f118 +0*0)
eq ?f113 *?f116 /1
eq ?f115 *?f114 /1
eq ?f120 +0-?f121
eq ?f25 *?f25 /1
eq ?f119 *?f119 /1
eq ?f123 *?f124 /1
eq ?f122 *?f122 /1
eq ?f125 +0-?f126
eq max(?f127 ,0)
eq sqrt(?f128 )
eq ?f87 *?f117 /1
eq ?f130 *?f129 /1
eq ?f122 *?f118 /1
eq (?f132 +?f131 )/?f124
eq ?f132 +0-?f131
eq ?f134 *1/?f124
eq abs(?f118 )
eq ?f136 *?f129 /1
eq ?f122 *?f117 /-1
eq (?f138 +?f137 )/?f124
eq ?f138 +0-?f137
eq ?f140 *1/?f124
eq ?f113 +0-?f133
eq ?f113 +0-?f135
eq ?f114 +0-?f139
eq ?f114 +0-?f141
eq sqrt(?f142 *?f142 +?f144 *?f144 +0*0)
eq sqrt(?f143 *?f143 +?f145 *?f145 +0*0)
eq ?f147 +0-?f146
eq if(?f148 ,?f133 ,?f135 )
eq if(?f148 ,?f139 ,?f141 )
eq ?f149 *?f13 /?f25
eq ?f150 *?f14 /?f25
eq ?f21 +?f151 -0
eq ?f23 +?f152 -0
eq ?f15 *sin(pi*(?f4 )/10800000)
eq ?f16 *cos(pi*(?f4 )/10800000)
eq ?f15 *(cos(atan2(?f155 ,?f156 )))
eq ?f16 *(sin(atan2(?f155 ,?f156 )))
eq ?f21 +?f157 -0
eq ?f23 +?f158 -0
eq ?f4 +0-?f48
eq ?f15 *sin(pi*(?f161 )/10800000)
eq ?f16 *cos(pi*(?f161 )/10800000)
eq ?f15 *(cos(atan2(?f162 ,?f163 )))
eq ?f16 *(sin(atan2(?f162 ,?f163 )))
eq ?f21 +?f164 -0
eq ?f23 +?f165 -0
eq ?f7 *cos(pi*(?f161 )/10800000)
eq ?f7 *sin(pi*(?f161 )/10800000)
eq ?f159 +?f168 -0
eq ?f160 +?f169 -0
eq ?f7 *cos(pi*(?f161 )/10800000)
eq ?f7 *sin(pi*(?f161 )/10800000)
eq ?f159 +0-?f172
eq ?f160 +0-?f173
eq ?f111 +0-?f153
eq ?f112 +0-?f154
eq sqrt(?f176 *?f176 +?f177 *?f177 +0*0)
eq ?f178 *1/2
eq ?f179 +0-?f7
eq if(?f180 ,?f111 ,?f58 )
eq if(?f180 ,?f112 ,?f59 )
eq if(?f180 ,?f153 ,?f62 )
eq if(?f180 ,?f154 ,?f63 )
eq (10800000*atan2(?f110 ,?f109 ))/pi
eq ?f185 +21600000-0
eq if(?f185 ,?f185 ,?f186 )
eq ?f187 +0-?f3
eq ?f188 +21600000-0
eq if(?f188 ,?f188 ,?f189 )
eq ?f4 +0-?f190
eq ?f191 +21600000-0
eq if(?f191 ,?f191 ,?f192 )
eq ?f187 +0-?f193
eq ?f194 +21600000-0
eq if(?f194 ,?f194 ,?f195 )
eq (10800000*atan2(?f152 ,?f151 ))/pi
eq ?f197 +21600000-0
eq if(?f197 ,?f197 ,?f198 )
eq ?f199 +0-?f3
eq ?f200 +0-21600000
eq if(?f200 ,?f201 ,?f200 )
eq ?f4 +0-?f202
eq ?f203 +0-21600000
eq if(?f204 ,?f204 ,?f203 )
eq ?f205 +0-?f199
eq ?f206 +0-21600000
eq if(?f206 ,?f207 ,?f206 )
eq ?f10 *sin(pi*(?f193 )/10800000)
eq ?f12 *cos(pi*(?f193 )/10800000)
eq ?f10 *(cos(atan2(?f209 ,?f210 )))
eq ?f12 *(sin(atan2(?f209 ,?f210 )))
eq ?f21 +?f211 -0
eq ?f23 +?f212 -0
eq ?f13 *sin(pi*(?f205 )/10800000)
eq ?f14 *cos(pi*(?f205 )/10800000)
eq ?f13 *(cos(atan2(?f215 ,?f216 )))
eq ?f14 *(sin(atan2(?f215 ,?f216 )))
eq ?f21 +?f217 -0
eq ?f23 +?f218 -0
eq if(?f180 ,?f213 ,?f170 )
eq if(?f180 ,?f214 ,?f171 )
eq if(?f180 ,?f219 ,?f174 )
eq if(?f180 ,?f220 ,?f175 )
eq ?f161 +0-5400000
eq ?f49 +5400000-0
eq ?f49 +10800000-0
eq ?f161 +10800000-0
eq ?f10 *cos(pi*(2700000)/10800000)
eq ?f12 *sin(pi*(2700000)/10800000)
eq ?f21 +0-?f229
eq ?f21 +?f229 -0
eq ?f23 +0-?f230
eq ?f23 +?f230 -0
eq (?f193 )/60000.0
eq (?f196 )/60000.0
eq (?f199 )/60000.0
eq (?f208 )/60000.0
path M ?f166 ?f167 L ?f221 ?f222 L ?f213 ?f214 G ?f10 ?f12 ?f235 ?f236 L ?f181 ?f182 L ?f54 ?f55 L ?f183 ?f184 L ?f153 ?f154 G ?f13 ?f14 ?f237 ?f238 L ?f223 ?f224 Z N

[leftRightRibbon]
mod adj1=50000 adj2=50000 adj3=16667
eq if(0-$2 ,0,if(33333-$2 ,$2 ,33333))
eq 100000+0-?f0
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq logwidth/2
eq logwidth/32
eq ?f3 +0-?f4
eq min(logwidth,logheight)
eq 100000*?f5 /?f6
eq if(0-$1 ,0,if(?f7 -$1 ,$1 ,?f7 ))
eq ?f6 *?f8 /100000
eq logwidth+0-?f9
eq logheight*?f2 /200000
eq logheight*?f0 /-200000
eq logheight/2
eq ?f13 +?f12 -?f11
eq ?f13 +?f11 -?f12
eq ?f14 +?f11 -0
eq logheight+0-?f16
eq ?f16 *2/1
eq logheight+0-?f18
eq ?f18 +0-?f14
eq logheight+0-?f20
eq ?f0 *?f6 /400000
eq logwidth/2
eq ?f23 +0-?f4
eq ?f23 +?f4 -0
eq ?f14 +?f22 -0
eq ?f21 +0-?f22
eq logwidth
eq logheight
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
path M 0 ?f16 L ?f9 0 L ?f9 ?f14 L ?f23 ?f14 G ?f4 ?f22 ?f30 ?f31 G ?f4 ?f22 ?f32 ?f33 L ?f10 ?f21 L ?f10 ?f19 L ?f28 ?f17 L ?f10 ?f29 L ?f10 ?f15 L ?f23 ?f15 G ?f4 ?f22 ?f34 ?f35 L ?f24 ?f20 L ?f9 ?f20 L ?f9 ?f18 Z S N M ?f25 ?f26 G ?f4 ?f22 ?f36 ?f37 G ?f4 ?f22 ?f38 ?f39 L ?f25 ?f21 Z I S N M 0 ?f16 L ?f9 0 L ?f9 ?f14 L ?f23 ?f14 G ?f4 ?f22 ?f40 ?f41 G ?f4 ?f22 ?f42 ?f43 L ?f10 ?f21 L ?f10 ?f19 L ?f28 ?f17 L ?f10 ?f29 L ?f10 ?f15 L ?f23 ?f15 G ?f4 ?f22 ?f44 ?f45 L ?f24 ?f20 L ?f9 ?f20 L ?f9 ?f18 Z M ?f25 ?f26 L ?f25 ?f21 M ?f24 ?f27 L ?f24 ?f20 F N

[leftRightUpArrow]
mod adj1=25000 adj2=25000 adj3=25000
eq if(0-$1 ,0,if(50000-$1 ,$1 ,50000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq 100000+0-?f1
eq ?f3 *1/2
eq if(0-$2 ,0,if(?f4 -$2 ,$2 ,?f4 ))
eq min(logwidth,logheight)
eq ?f6 *?f5 /100000
eq ?f6 *?f0 /100000
eq logwidth/2
eq ?f9 +0-?f8
eq ?f9 +?f8 -0
eq ?f6 *?f2 /200000
eq ?f9 +0-?f12
eq ?f9 +?f12 -0
eq logwidth+0-?f7
eq ?f6 *?f0 /50000
eq logheight+0-?f16
eq logheight+0-?f8
eq ?f18 +0-?f12
eq ?f18 +?f12 -0
eq ?f12 *?f7 /?f8
eq logwidth+0-?f21
eq logwidth
eq logheight
path M 0 ?f18 L ?f7 ?f17 L ?f7 ?f19 L ?f13 ?f19 L ?f13 ?f7 L ?f10 ?f7 L ?f9 0 L ?f11 ?f7 L ?f14 ?f7 L ?f14 ?f19 L ?f15 ?f19 L ?f15 ?f17 L ?f23 ?f18 L ?f15 ?f24 L ?f15 ?f20 L ?f7 ?f20 L ?f7 ?f24 Z N

[leftUpArrow]
mod adj1=25000 adj2=25000 adj3=25000
eq if(0-$1 ,0,if(50000-$1 ,$1 ,50000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq 100000+0-?f1
eq if(0-$2 ,0,if(?f3 -$2 ,$2 ,?f3 ))
eq min(logwidth,logheight)
eq ?f5 *?f4 /100000
eq ?f5 *?f0 /50000
eq logwidth+0-?f7
eq logheight+0-?f7
eq ?f5 *?f0 /100000
eq logwidth+0-?f10
eq logheight+0-?f10
eq ?f5 *?f2 /200000
eq ?f11 +0-?f13
eq ?f11 +?f13 -0
eq ?f12 +0-?f13
eq ?f12 +?f13 -0
eq ?f13 *?f6 /?f10
eq (?f6 +?f15 )/2
eq (?f6 +?f17 )/2
eq logheight
eq logwidth
path M 0 ?f12 L ?f6 ?f9 L ?f6 ?f16 L ?f14 ?f16 L ?f14 ?f6 L ?f8 ?f6 L ?f11 0 L ?f22 ?f6 L ?f15 ?f6 L ?f15 ?f17 L ?f6 ?f17 L ?f6 ?f21 Z N

[line]
eq logwidth
eq logheight
path M 0 0 L ?f0 ?f1 N

[lineInv]
eq logheight
eq logwidth
path M 0 ?f0 L ?f1 0 N

[mathDivide]
mod adj1=23520 adj2=5880 adj3=11760
eq if(1000-$0 ,1000,if(36745-$0 ,$0 ,36745))
eq 0+0-?f0
eq (73490+?f1 )/4
eq 36745*logwidth/logheight
eq min(?f2 ,?f3 )
eq if(1000-$2 ,1000,if(?f4 -$2 ,$2 ,?f4 ))
eq -4*?f5 /1
eq 73490+?f6 -?f0
eq if(0-$1 ,0,if(?f7 -$1 ,$1 ,?f7 ))
eq logheight*?f0 /200000
eq logheight*?f8 /100000
eq logheight*?f5 /100000
eq logwidth*73490/200000
eq logheight/2
eq ?f13 +0-?f9
eq ?f13 +?f9 -0
eq ?f10 +?f11 -0
eq ?f14 +0-?f16
eq ?f17 +0-?f11
eq logheight+0-?f18
eq logwidth/2
eq ?f20 +0-?f12
eq ?f20 +?f12 -0
eq ?f20 +0-?f11
eq logwidth
eq (16200000)/60000.0
eq (21600000)/60000.0
eq (5400000)/60000.0
eq (21600000)/60000.0
path M ?f20 ?f18 G ?f11 ?f11 ?f25 ?f26 Z M ?f20 ?f19 G ?f11 ?f11 ?f27 ?f28 Z M ?f21 ?f14 L ?f22 ?f14 L ?f22 ?f15 L ?f21 ?f15 Z N

[mathNotEqual]
mod adj1=23520 adj2=6600000 adj3=11760
eq if(0-$0 ,0,if(50000-$0 ,$0 ,50000))
eq if(4200000-$1 ,4200000,if(6600000-$1 ,$1 ,6600000))
eq ?f0 *2/1
eq 100000+0-2
eq if(0-$2 ,0,if(?f3 -$2 ,$2 ,?f3 ))
eq logheight*?f0 /100000
eq logheight*?f4 /200000
eq logwidth*73490/200000
eq logwidth/2
eq ?f8 +0-?f7
eq ?f8 +?f7 -0
eq logheight/2
eq ?f11 +0-?f6
eq ?f11 +?f6 -0
eq ?f12 +0-?f5
eq ?f13 +?f5 -0
eq ?f1 +0-5400000
eq logheight/2
eq ?f17 *tan(pi*(?f16 )/10800000)
eq sqrt(?f18 *?f18 +?f17 *?f17 +0*0)
eq ?f19 *?f5 /?f17
eq ?f20 *1/2
eq ?f8 +?f18 -?f21
eq ?f18 *?f14 /?f17
eq ?f22 +0-?f23
eq ?f18 *?f12 /?f17
eq ?f22 +0-?f25
eq ?f18 *?f13 /?f17
eq ?f22 +0-?f27
eq ?f18 *?f15 /?f17
eq ?f22 +0-?f29
eq ?f18 *2/1
eq ?f22 +0-?f31
eq ?f22 +?f20 -0
eq ?f24 +?f20 -0
eq ?f26 +?f20 -0
eq ?f28 +?f20 -0
eq ?f30 +?f20 -0
eq ?f32 +?f20 -0
eq ?f5 *?f17 /?f19
eq ?f22 +?f39 -0
eq ?f33 +0-?f39
eq if(?f16 ,?f40 ,?f33 )
eq if(?f16 ,?f22 ,?f41 )
eq ?f5 *?f18 /?f19
eq 0+0-?f44
eq if(?f16 ,?f44 ,0)
eq if(?f16 ,0,?f45 )
eq logwidth+0-?f42
eq logwidth+0-?f43
eq logheight+0-?f46
eq logheight+0-?f47
eq (?f42 +?f43 )/2
eq (?f49 +?f48 )/2
eq (?f46 +?f47 )/2
eq (?f14 +?f12 )/2
eq (?f13 +?f15 )/2
eq (?f51 +?f50 )/2
eq logwidth
path M ?f9 ?f14 L ?f24 ?f14 L ?f43 ?f47 L ?f42 ?f46 L ?f34 ?f14 L ?f10 ?f14 L ?f10 ?f12 L ?f35 ?f12 L ?f36 ?f13 L ?f10 ?f13 L ?f10 ?f15 L ?f37 ?f15 L ?f49 ?f51 L ?f48 ?f50 L ?f30 ?f15 L ?f9 ?f15 L ?f9 ?f13 L ?f28 ?f13 L ?f26 ?f12 L ?f9 ?f12 Z N

[noSmoking]
mod adj=18750
eq if(0-$0 ,0,if(50000-$0 ,$0 ,50000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq logwidth/2
eq ?f3 +0-?f2
eq logheight/2
eq ?f5 +0-?f2
eq (10800000*atan2(logheight,logwidth))/pi
eq ?f6 *cos(pi*(?f7 )/10800000)
eq ?f4 *sin(pi*(?f7 )/10800000)
eq sqrt(?f8 *?f8 +?f9 *?f9 +0*0)
eq ?f4 *?f6 /?f10
eq ?f2 *1/2
eq (10800000*atan2(?f12 ,?f11 ))/pi
eq ?f13 *2/1
eq -10800000+?f14 -0
eq (10800000*atan2(logheight,logwidth))/pi
eq ?f16 +0-?f13
eq ?f17 +0-10800000
eq ?f6 *cos(pi*(?f17 )/10800000)
eq ?f4 *sin(pi*(?f17 )/10800000)
eq sqrt(?f19 *?f19 +?f20 *?f20 +0*0)
eq ?f4 *?f6 /?f21
eq ?f22 *cos(pi*(?f17 )/10800000)
eq ?f22 *sin(pi*(?f17 )/10800000)
eq logwidth/2
eq ?f25 +?f23 -0
eq logheight/2
eq ?f27 +?f24 -0
eq ?f25 +0-?f23
eq ?f27 +0-?f24
eq ?f3 *cos(pi*(2700000)/10800000)
eq ?f5 *sin(pi*(2700000)/10800000)
eq ?f25 +0-?f31
eq ?f25 +?f31 -0
eq ?f27 +0-?f32
eq ?f27 +?f32 -0
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (?f17 )/60000.0
eq (?f15 )/60000.0
eq (?f18 )/60000.0
eq (?f15 )/60000.0
path M 0 ?f27 G ?f3 ?f5 ?f39 ?f40 G ?f3 ?f5 ?f41 ?f42 G ?f3 ?f5 ?f43 ?f44 G ?f3 ?f5 ?f45 ?f46 Z M ?f26 ?f28 G ?f4 ?f6 ?f47 ?f48 Z M ?f29 ?f30 G ?f4 ?f6 ?f49 ?f50 Z N

[nonIsoscelesTrapezoid]
mod adj1=25000 adj2=25000
eq min(logwidth,logheight)
eq 50000*logwidth/?f0
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f0 *?f2 /200000
eq ?f0 *?f2 /100000
eq ?f0 *?f3 /100000
eq logwidth+0-?f6
eq (logwidth+?f7 )/2
eq logwidth/3
eq ?f9 *?f2 /?f1
eq max(?f2 ,?f3 )
eq logheight/3
eq ?f12 *?f11 /?f1
eq ?f9 *?f3 /?f1
eq logwidth+0-?f14
eq logheight/2
eq logwidth/2
eq logheight
eq logwidth
path M 0 ?f18 L ?f5 0 L ?f7 0 L ?f19 ?f18 Z N

[pieWedge]
eq logwidth*cos(pi*(13500000)/10800000)
eq logheight*sin(pi*(13500000)/10800000)
eq logwidth+?f0 -0
eq logheight+?f1 -0
eq logwidth
eq logheight/2
eq logwidth/2
eq logheight
eq logwidth
eq logheight
eq (10800000)/60000.0
eq (5400000)/60000.0
path M 0 ?f7 G ?f8 ?f9 ?f10 ?f11 L ?f4 ?f7 Z N

[plaqueTabs]
eq sqrt(logwidth*logwidth+logheight*logheight+0*0)
eq 1*?f0 /20
eq 0+logheight-?f1
eq 0+logwidth-?f1
eq logheight
eq logwidth
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
path M 0 0 L ?f1 0 G ?f1 ?f1 ?f6 ?f7 Z N M 0 ?f2 G ?f1 ?f1 ?f8 ?f9 L 0 ?f4 Z N M ?f5 0 L ?f5 ?f1 G ?f1 ?f1 ?f10 ?f11 Z N M ?f3 ?f4 G ?f1 ?f1 ?f12 ?f13 L ?f5 ?f4 Z N

[quadArrowCallout]
mod adj1=18515 adj2=18515 adj3=18515 adj4=48123
eq if(0-$1 ,0,if(50000-$1 ,$1 ,50000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq 50000+0-?f0
eq if(0-$2 ,0,if(?f3 -$2 ,$2 ,?f3 ))
eq ?f4 *2/1
eq 100000+0-?f5
eq if(?f2 -$3 ,?f2 ,if(?f6 -$3 ,$3 ,?f6 ))
eq min(logwidth,logheight)
eq ?f8 *?f0 /100000
eq ?f8 *?f2 /200000
eq ?f8 *?f4 /100000
eq logwidth*?f7 /200000
eq logheight*?f7 /200000
eq logwidth+0-?f11
eq logwidth/2
eq ?f15 +0-?f12
eq ?f15 +?f12 -0
eq ?f15 +0-?f9
eq ?f15 +?f9 -0
eq ?f15 +0-?f10
eq ?f15 +?f10 -0
eq logheight+0-?f11
eq logheight/2
eq ?f23 +0-?f13
eq ?f23 +?f13 -0
eq ?f23 +0-?f9
eq ?f23 +?f9 -0
eq ?f23 +0-?f10
eq ?f23 +?f10 -0
eq logwidth
eq logheight
path M 0 ?f23 L ?f11 ?f26 L ?f11 ?f28 L ?f16 ?f28 L ?f16 ?f24 L ?f20 ?f24 L ?f20 ?f11 L ?f18 ?f11 L ?f15 0 L ?f19 ?f11 L ?f21 ?f11 L ?f21 ?f24 L ?f17 ?f24 L ?f17 ?f28 L ?f14 ?f28 L ?f14 ?f26 L ?f30 ?f23 L ?f14 ?f27 L ?f14 ?f29 L ?f17 ?f29 L ?f17 ?f25 L ?f21 ?f25 L ?f21 ?f22 L ?f19 ?f22 L ?f15 ?f31 L ?f18 ?f22 L ?f20 ?f22 L ?f20 ?f25 L ?f16 ?f25 L ?f16 ?f29 L ?f11 ?f29 L ?f11 ?f27 Z N

[ribbon]
mod adj1=16667 adj2=50000
eq if(0-$0 ,0,if(33333-$0 ,$0 ,33333))
eq if(25000-$1 ,25000,if(75000-$1 ,$1 ,75000))
eq logwidth/8
eq logwidth+0-?f2
eq logwidth*?f1 /200000
eq logwidth/2
eq ?f5 +0-?f4
eq ?f5 +?f4 -0
eq logwidth/32
eq ?f6 +?f8 -0
eq ?f7 +0-?f8
eq ?f6 +?f2 -0
eq ?f7 +0-?f2
eq ?f11 +0-?f8
eq ?f12 +?f8 -0
eq logheight*?f0 /200000
eq logheight*?f0 /100000
eq logheight+0-?f16
eq ?f17 *1/2
eq logheight*?f0 /400000
eq logheight+0-?f19
eq ?f16 +0-?f19
eq logheight
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq logwidth
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
path M 0 0 L ?f13 0 G ?f8 ?f19 ?f23 ?f24 L ?f9 ?f15 G ?f8 ?f19 ?f25 ?f26 L ?f10 ?f16 G ?f8 ?f19 ?f27 ?f28 L ?f14 ?f15 G ?f8 ?f19 ?f29 ?f30 L ?f31 0 L ?f3 ?f18 L ?f31 ?f17 L ?f7 ?f17 L ?f7 ?f20 G ?f8 ?f19 ?f32 ?f33 L ?f9 ?f22 G ?f8 ?f19 ?f34 ?f35 L ?f6 ?f17 L 0 ?f17 L ?f2 ?f18 Z S N M ?f11 ?f19 G ?f8 ?f19 ?f36 ?f37 L ?f9 ?f15 G ?f8 ?f19 ?f38 ?f39 L ?f11 ?f16 Z M ?f12 ?f19 G ?f8 ?f19 ?f40 ?f41 L ?f10 ?f15 G ?f8 ?f19 ?f42 ?f43 L ?f12 ?f16 Z I S N M 0 0 L ?f13 0 G ?f8 ?f19 ?f44 ?f45 L ?f9 ?f15 G ?f8 ?f19 ?f46 ?f47 L ?f10 ?f16 G ?f8 ?f19 ?f48 ?f49 L ?f14 ?f15 G ?f8 ?f19 ?f50 ?f51 L ?f31 0 L ?f3 ?f18 L ?f31 ?f17 L ?f7 ?f17 L ?f7 ?f20 G ?f8 ?f19 ?f52 ?f53 L ?f9 ?f22 G ?f8 ?f19 ?f54 ?f55 L ?f6 ?f17 L 0 ?f17 L ?f2 ?f18 Z M ?f11 ?f19 L ?f11 ?f16 M ?f12 ?f16 L ?f12 ?f19 M ?f6 ?f17 L ?f6 ?f21 M ?f7 ?f21 L ?f7 ?f17 F N

[ribbon2]
mod adj1=16667 adj2=50000
eq if(0-$0 ,0,if(33333-$0 ,$0 ,33333))
eq if(25000-$1 ,25000,if(75000-$1 ,$1 ,75000))
eq logwidth/8
eq logwidth+0-?f2
eq logwidth*?f1 /200000
eq logwidth/2
eq ?f5 +0-?f4
eq ?f5 +?f4 -0
eq logwidth/32
eq ?f6 +?f8 -0
eq ?f7 +0-?f8
eq ?f6 +?f2 -0
eq ?f7 +0-?f2
eq ?f11 +0-?f8
eq ?f12 +?f8 -0
eq logheight*?f0 /200000
eq logheight+0-?f15
eq logheight*?f0 /100000
eq logheight+0-?f17
eq 0+?f17 -0
eq (?f19 +logheight)/2
eq logheight*?f0 /400000
eq logheight+0-?f21
eq ?f16 +0-?f21
eq logheight
eq (5400000)/60000.0
eq (-10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq logwidth
eq (0)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (-10800000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (-10800000)/60000.0
eq (16200000)/60000.0
eq (-10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
path M 0 ?f24 L ?f13 ?f24 G ?f8 ?f21 ?f25 ?f26 L ?f9 ?f16 G ?f8 ?f21 ?f27 ?f28 L ?f10 ?f18 G ?f8 ?f21 ?f29 ?f30 L ?f14 ?f16 G ?f8 ?f21 ?f31 ?f32 L ?f33 ?f24 L ?f3 ?f20 L ?f33 ?f19 L ?f7 ?f19 L ?f7 ?f21 G ?f8 ?f21 ?f34 ?f35 L ?f9 0 G ?f8 ?f21 ?f36 ?f37 L ?f6 ?f19 L 0 ?f19 L ?f2 ?f20 Z S N M ?f11 ?f22 G ?f8 ?f21 ?f38 ?f39 L ?f9 ?f16 G ?f8 ?f21 ?f40 ?f41 L ?f11 ?f18 Z M ?f12 ?f22 G ?f8 ?f21 ?f42 ?f43 L ?f10 ?f16 G ?f8 ?f21 ?f44 ?f45 L ?f12 ?f18 Z I S N M 0 ?f24 L ?f2 ?f20 L 0 ?f19 L ?f6 ?f19 L ?f6 ?f21 G ?f8 ?f21 ?f46 ?f47 L ?f10 0 G ?f8 ?f21 ?f48 ?f49 L ?f7 ?f19 L ?f7 ?f19 L ?f33 ?f19 L ?f3 ?f20 L ?f33 ?f24 L ?f14 ?f24 G ?f8 ?f21 ?f50 ?f51 L ?f10 ?f16 G ?f8 ?f21 ?f52 ?f53 L ?f9 ?f18 G ?f8 ?f21 ?f54 ?f55 L ?f13 ?f16 G ?f8 ?f21 ?f56 ?f57 Z M ?f11 ?f18 L ?f11 ?f22 M ?f12 ?f22 L ?f12 ?f18 M ?f6 ?f23 L ?f6 ?f19 M ?f7 ?f19 L ?f7 ?f23 F N

[rightArrowCallout]
mod adj1=25000 adj2=25000 adj3=25000 adj4=64977
eq min(logwidth,logheight)
eq 50000*logheight/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f2 *2/1
eq if(0-$0 ,0,if(?f3 -$0 ,$0 ,?f3 ))
eq 100000*logwidth/?f0
eq if(0-$2 ,0,if(?f5 -$2 ,$2 ,?f5 ))
eq ?f6 *?f0 /logwidth
eq 100000+0-?f7
eq if(0-$3 ,0,if(?f8 -$3 ,$3 ,?f8 ))
eq ?f0 *?f2 /100000
eq ?f0 *?f4 /200000
eq logheight/2
eq ?f12 +0-?f10
eq ?f12 +0-?f11
eq ?f12 +?f11 -0
eq ?f12 +?f10 -0
eq ?f0 *?f6 /100000
eq logwidth+0-?f17
eq logwidth*?f9 /100000
eq ?f19 *1/2
eq logwidth
eq logheight
path M 0 0 L ?f19 0 L ?f19 ?f14 L ?f18 ?f14 L ?f18 ?f13 L ?f21 ?f12 L ?f18 ?f16 L ?f18 ?f15 L ?f19 ?f15 L ?f19 ?f22 L 0 ?f22 Z N

[rightBrace]
mod adj1=8333 adj2=50000
eq if(0-$1 ,0,if(100000-$1 ,$1 ,100000))
eq 100000+0-?f0
eq min(?f1 ,?f0 )
eq ?f2 *1/2
eq min(logwidth,logheight)
eq ?f3 *logheight/?f4
eq if(0-$0 ,0,if(?f5 -$0 ,$0 ,?f5 ))
eq ?f4 *?f6 /100000
eq logheight*?f0 /100000
eq ?f8 +0-?f7
eq logheight+0-?f7
eq logwidth/2
eq ?f11 *cos(pi*(2700000)/10800000)
eq ?f7 *sin(pi*(2700000)/10800000)
eq 0+?f12 -0
eq ?f7 +0-?f13
eq logheight+?f13 -?f7
eq logwidth/2
eq logwidth
eq logheight
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
path M 0 0 G ?f11 ?f7 ?f20 ?f21 L ?f17 ?f9 G ?f11 ?f7 ?f22 ?f23 G ?f11 ?f7 ?f24 ?f25 L ?f17 ?f10 G ?f11 ?f7 ?f26 ?f27 Z S N M 0 0 G ?f11 ?f7 ?f28 ?f29 L ?f17 ?f9 G ?f11 ?f7 ?f30 ?f31 G ?f11 ?f7 ?f32 ?f33 L ?f17 ?f10 G ?f11 ?f7 ?f34 ?f35 F N

[smileyFace]
mod adj=4653
eq if(-4653-$0 ,-4653,if(4653-$0 ,$0 ,4653))
eq logwidth*4969/21699
eq logwidth*6215/21600
eq logwidth*13135/21600
eq logwidth*16640/21600
eq logheight*7570/21600
eq logheight*16515/21600
eq logheight*?f0 /100000
eq ?f6 +0-?f7
eq ?f6 +?f7 -0
eq logheight*?f0 /50000
eq ?f9 +?f10 -0
eq logwidth/2
eq ?f12 *cos(pi*(2700000)/10800000)
eq logheight/2
eq ?f14 *sin(pi*(2700000)/10800000)
eq logwidth/2
eq ?f16 +0-?f13
eq ?f16 +?f13 -0
eq logheight/2
eq ?f19 +0-?f15
eq ?f19 +?f15 -0
eq logwidth*1125/21600
eq logheight*1125/21600
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (21600000)/60000.0
eq (10800000)/60000.0
eq (21600000)/60000.0
eq (10800000)/60000.0
eq (21600000)/60000.0
eq (10800000)/60000.0
eq (21600000)/60000.0
path M 0 ?f19 G ?f12 ?f14 ?f26 ?f27 Z S N M ?f2 ?f5 G ?f22 ?f23 ?f28 ?f29 M ?f3 ?f5 G ?f22 ?f23 ?f30 ?f31 I N M ?f1 ?f8 Q ?f16 ?f11 ?f4 ?f8 F N M 0 ?f19 G ?f12 ?f14 ?f32 ?f33 Z F N

[squareTabs]
eq sqrt(logwidth*logwidth+logheight*logheight+0*0)
eq 1*?f0 /20
eq 0+logheight-?f1
eq 0+logwidth-?f1
eq logheight
eq logwidth
path M 0 0 L ?f1 0 L ?f1 ?f1 L 0 ?f1 Z N M 0 ?f2 L ?f1 ?f2 L ?f1 ?f4 L 0 ?f4 Z N M ?f3 0 L ?f5 0 L ?f5 ?f1 L ?f3 ?f1 Z N M ?f3 ?f2 L ?f5 ?f2 L ?f5 ?f4 L ?f3 ?f4 Z N

[straightConnector1]
eq logwidth
eq logheight
path M 0 0 L ?f0 ?f1 F N

[stripedRightArrow]
mod adj1=50000 adj2=50000
eq min(logwidth,logheight)
eq 84375*logwidth/?f0
eq if(0-$0 ,0,if(100000-$0 ,$0 ,100000))
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f0 *5/32
eq ?f0 *?f3 /100000
eq logwidth+0-?f5
eq logheight*?f2 /200000
eq logheight/2
eq ?f8 +0-?f7
eq ?f8 +?f7 -0
eq logheight/2
eq ?f7 *?f5 /?f11
eq logwidth+0-?f12
eq logheight
eq logwidth
eq min(logwidth,logheight)/32
eq min(logwidth,logheight)/16
eq min(logwidth,logheight)/8
path M 0 ?f9 L ?f16 ?f9 L ?f16 ?f10 L 0 ?f10 Z M ?f17 ?f9 L ?f18 ?f9 L ?f18 ?f10 L ?f17 ?f10 Z M ?f4 ?f9 L ?f6 ?f9 L ?f6 0 L ?f15 ?f8 L ?f6 ?f14 L ?f6 ?f10 L ?f4 ?f10 Z N

[sun]
mod adj=25000
eq if(12500-$0 ,12500,if(46875-$0 ,$0 ,46875))
eq 50000+0-?f0
eq ?f1 *30274/32768
eq ?f1 *12540/32768
eq ?f2 +50000-0
eq ?f3 +50000-0
eq 50000+0-?f2
eq 50000+0-?f3
eq ?f1 *23170/32768
eq 50000+?f8 -0
eq 50000+0-?f8
eq ?f6 *3/4
eq ?f7 *3/4
eq ?f11 +3662-0
eq ?f12 +3662-0
eq ?f12 +12500-0
eq 100000+0-?f11
eq 100000+0-?f13
eq 100000+0-?f14
eq 100000+0-?f15
eq logwidth*18436/21600
eq logheight*3163/21600
eq logwidth*3163/21600
eq logheight*18436/21600
eq logwidth*?f9 /100000
eq logwidth*?f10 /100000
eq logwidth*?f11 /100000
eq logwidth*?f13 /100000
eq logwidth*?f14 /100000
eq logwidth*?f15 /100000
eq logwidth*?f16 /100000
eq logwidth*?f17 /100000
eq logwidth*?f18 /100000
eq logwidth*?f19 /100000
eq logwidth*?f0 /100000
eq logwidth*?f1 /100000
eq logheight*?f1 /100000
eq logheight*?f9 /100000
eq logheight*?f10 /100000
eq logheight*?f11 /100000
eq logheight*?f13 /100000
eq logheight*?f14 /100000
eq logheight*?f15 /100000
eq logheight*?f16 /100000
eq logheight*?f17 /100000
eq logheight*?f18 /100000
eq logheight*?f19 /100000
eq logheight/2
eq logwidth/2
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (21600000)/60000.0
path M ?f50 ?f47 L ?f30 ?f46 L ?f30 ?f42 Z M ?f20 ?f21 L ?f31 ?f41 L ?f32 ?f40 Z M ?f48 0 L ?f33 ?f39 L ?f29 ?f39 Z M ?f22 ?f21 L ?f28 ?f40 L ?f27 ?f41 Z M 0 ?f47 L ?f26 ?f42 L ?f26 ?f46 Z M ?f22 ?f23 L ?f27 ?f45 L ?f28 ?f44 Z M ?f48 ?f49 L ?f29 ?f43 L ?f33 ?f43 Z M ?f20 ?f23 L ?f32 ?f44 L ?f31 ?f45 Z M ?f34 ?f47 G ?f35 ?f36 ?f51 ?f52 Z N

[swooshArrow]
mod adj1=25000 adj2=16667
eq if(1-$0 ,1,if(75000-$0 ,$0 ,75000))
eq min(logwidth,logheight)
eq 70000*logwidth/?f1
eq if(0-$1 ,0,if(?f2 -$1 ,$1 ,?f2 ))
eq logheight*?f0 /100000
eq ?f1 *?f3 /100000
eq logwidth+0-?f5
eq min(logwidth,logheight)/8
eq 0+?f7 -0
eq 5400000*1/14
eq ?f7 *tan(pi*(?f9 )/10800000)
eq ?f6 +0-?f10
eq ?f4 *tan(pi*(?f9 )/10800000)
eq ?f8 +?f4 -0
eq ?f6 +?f12 -0
eq ?f14 +?f10 -0
eq ?f13 +?f7 -0
eq ?f16 +0-0
eq ?f17 *1/2
eq logheight*1/20
eq 0+?f18 -?f19
eq logheight/6
eq ?f21 *1/1
eq ?f21 +?f22 -0
eq logwidth/6
eq ?f24
eq ?f21 *1/2
eq ?f13 +?f26 -0
eq logwidth/4
eq ?f28
eq logheight
eq logwidth
path M 0 ?f30 Q ?f25 ?f23 ?f6 ?f8 L ?f11 0 L ?f31 ?f20 L ?f15 ?f16 L ?f14 ?f13 Q ?f29 ?f27 0 ?f30 Z N

[upArrowCallout]
mod adj1=25000 adj2=25000 adj3=25000 adj4=64977
eq min(logwidth,logheight)
eq 50000*logwidth/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f2 *2/1
eq if(0-$0 ,0,if(?f3 -$0 ,$0 ,?f3 ))
eq 100000*logheight/?f0
eq if(0-$2 ,0,if(?f5 -$2 ,$2 ,?f5 ))
eq ?f6 *?f0 /logheight
eq 100000+0-?f7
eq if(0-$3 ,0,if(?f8 -$3 ,$3 ,?f8 ))
eq ?f0 *?f2 /100000
eq ?f0 *?f4 /200000
eq logwidth/2
eq ?f12 +0-?f10
eq ?f12 +0-?f11
eq ?f12 +?f11 -0
eq ?f12 +?f10 -0
eq ?f0 *?f6 /100000
eq logheight*?f9 /100000
eq logheight+0-?f18
eq (?f19 +logheight)/2
eq logwidth
eq logheight
path M 0 ?f19 L ?f14 ?f19 L ?f14 ?f17 L ?f13 ?f17 L ?f12 0 L ?f16 ?f17 L ?f15 ?f17 L ?f15 ?f19 L ?f21 ?f19 L ?f21 ?f22 L 0 ?f22 Z N

[upDownArrowCallout]
mod adj1=25000 adj2=25000 adj3=25000 adj4=48123
eq min(logwidth,logheight)
eq 50000*logwidth/?f0
eq if(0-$1 ,0,if(?f1 -$1 ,$1 ,?f1 ))
eq ?f2 *2/1
eq if(0-$0 ,0,if(?f3 -$0 ,$0 ,?f3 ))
eq 50000*logheight/?f0
eq if(0-$2 ,0,if(?f5 -$2 ,$2 ,?f5 ))
eq logheight/2
eq ?f6 *?f0 /?f7
eq 100000+0-?f8
eq if(0-$3 ,0,if(?f9 -$3 ,$3 ,?f9 ))
eq ?f0 *?f2 /100000
eq ?f0 *?f4 /200000
eq logwidth/2
eq ?f13 +0-?f11
eq ?f13 +0-?f12
eq ?f13 +?f12 -0
eq ?f13 +?f11 -0
eq ?f0 *?f6 /100000
eq logheight+0-?f18
eq logheight*?f10 /200000
eq logheight/2
eq ?f21 +0-?f20
eq ?f21 +?f20 -0
eq logwidth
eq logheight
path M 0 ?f22 L ?f15 ?f22 L ?f15 ?f18 L ?f14 ?f18 L ?f13 0 L ?f17 ?f18 L ?f16 ?f18 L ?f16 ?f22 L ?f24 ?f22 L ?f24 ?f23 L ?f16 ?f23 L ?f16 ?f19 L ?f17 ?f19 L ?f13 ?f25 L ?f14 ?f19 L ?f15 ?f19 L ?f15 ?f23 L 0 ?f23 Z N

[uturnArrow]
mod adj1=25000 adj2=25000 adj3=25000 adj4=43750 adj5=75000
eq if(0-$1 ,0,if(25000-$1 ,$1 ,25000))
eq ?f0 *2/1
eq if(0-$0 ,0,if(?f1 -$0 ,$0 ,?f1 ))
eq min(logwidth,logheight)
eq ?f2 *?f3 /logheight
eq 100000+0-?f4
eq ?f5 *logheight/?f3
eq if(0-$2 ,0,if(?f6 -$2 ,$2 ,?f6 ))
eq ?f7 +?f2 -0
eq ?f8 *?f3 /logheight
eq if(?f9 -$4 ,?f9 ,if(100000-$4 ,$4 ,100000))
eq ?f3 *?f2 /100000
eq ?f3 *?f0 /100000
eq ?f11 *1/2
eq ?f12 +0-?f13
eq logheight*?f10 /100000
eq ?f3 *?f7 /100000
eq ?f15 +0-?f16
eq logwidth+0-?f14
eq ?f18 *1/2
eq min(?f19 ,?f17 )
eq ?f20 *100000/?f3
eq if(0-$3 ,0,if(?f21 -$3 ,$3 ,?f21 ))
eq ?f3 *?f22 /100000
eq ?f23 +0-?f11
eq max(?f24 ,0)
eq ?f11 +?f25 -0
eq logwidth+0-?f12
eq ?f27 +0-?f12
eq ?f28 +?f14 -0
eq ?f18 +0-?f23
eq ?f29 +0-?f25
eq (?f11 +?f29 )/2
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (-5400000)/60000.0
eq (16200000)/60000.0
eq (-5400000)/60000.0
path M 0 ?f33 L 0 ?f23 G ?f23 ?f23 ?f35 ?f36 L ?f30 0 G ?f23 ?f23 ?f37 ?f38 L ?f18 ?f17 L ?f34 ?f17 L ?f27 ?f15 L ?f28 ?f17 L ?f29 ?f17 L ?f29 ?f26 G ?f25 ?f25 ?f39 ?f40 L ?f26 ?f11 G ?f25 ?f25 ?f41 ?f42 L ?f11 ?f33 Z N

[verticalScroll]
mod adj=12500
eq if(0-$0 ,0,if(25000-$0 ,$0 ,25000))
eq min(logwidth,logheight)
eq ?f1 *?f0 /100000
eq ?f2 *1/2
eq ?f2 *1/4
eq ?f2 +?f3 -0
eq ?f2 +?f2 -0
eq logwidth+0-?f2
eq logwidth+0-?f3
eq ?f7 +0-?f3
eq logheight+0-?f2
eq logheight+0-?f3
eq logwidth/2
eq logheight/2
eq logheight
eq (5400000)/60000.0
eq (-5400000)/60000.0
eq (5400000)/60000.0
eq (-10800000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (16200000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (10800000)/60000.0
eq (16200000)/60000.0
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (-5400000)/60000.0
path M ?f3 ?f14 G ?f3 ?f3 ?f15 ?f16 L ?f3 ?f11 G ?f4 ?f4 ?f17 ?f18 L ?f2 ?f10 L ?f2 ?f3 G ?f3 ?f3 ?f19 ?f20 L ?f8 0 G ?f3 ?f3 ?f21 ?f22 L ?f7 ?f2 L ?f7 ?f11 G ?f3 ?f3 ?f23 ?f24 Z M ?f6 ?f3 G ?f3 ?f3 ?f25 ?f26 G ?f4 ?f4 ?f27 ?f28 Z S N M ?f6 ?f3 G ?f3 ?f3 ?f29 ?f30 G ?f4 ?f4 ?f31 ?f32 Z M ?f2 ?f11 G ?f3 ?f3 ?f33 ?f34 G ?f4 ?f4 ?f35 ?f36 Z I S N M ?f2 ?f10 L ?f2 ?f3 G ?f3 ?f3 ?f37 ?f38 L ?f8 0 G ?f3 ?f3 ?f39 ?f40 L ?f7 ?f2 L ?f7 ?f11 G ?f3 ?f3 ?f41 ?f42 L ?f3 ?f14 G ?f3 ?f3 ?f43 ?f44 Z M ?f5 0 G ?f3 ?f3 ?f45 ?f46 G ?f4 ?f4 ?f47 ?f48 L ?f6 ?f3 M ?f7 ?f2 L ?f5 ?f2 M ?f3 ?f10 G ?f4 ?f4 ?f49 ?f50 L ?f2 ?f11 M ?f3 ?f14 G ?f3 ?f3 ?f51 ?f52 L ?f2 ?f10 F N

[wave]
mod adj1=12500 adj2=0
eq if(0-$0 ,0,if(20000-$0 ,$0 ,20000))
eq if(-10000-$1 ,-10000,if(10000-$1 ,$1 ,10000))
eq logheight*?f0 /100000
eq ?f2 *10/3
eq ?f2 +0-?f3
eq ?f2 +?f3 -0
eq logheight+0-?f2
eq ?f6 +0-?f3
eq ?f6 +?f3 -0
eq logwidth*?f1 /100000
eq logwidth*?f1 /50000
eq abs(?f9 )
eq if(?f10 ,0,?f10 )
eq 0+0-?f12
eq if(?f10 ,?f10 ,0)
eq logwidth+0-?f14
eq (?f12 +?f15 )/3
eq ?f13 +?f16 -0
eq (?f17 +?f15 )/2
eq 0+?f14 -0
eq logwidth+?f12 -0
eq ?f19 +?f16 -0
eq (?f21 +?f20 )/2
eq logwidth+0-?f11
eq logwidth/2
eq ?f24 +?f9 -0
eq ?f24 +0-?f9
eq max(?f13 ,?f19 )
eq min(?f15 ,?f20 )
eq logheight*?f0 /50000
eq logheight+0-?f29
eq logheight
eq logheight/2
path M ?f13 ?f2 C ?f17 ?f4 ?f18 ?f5 ?f15 ?f2 L ?f20 ?f6 C ?f22 ?f8 ?f21 ?f7 ?f19 ?f6 Z N

[wedgeRoundRectCallout]
mod adj1=-20833 adj2=62500 adj3=16667
eq logwidth*$0 /100000
eq logheight*$1 /100000
eq logwidth/2
eq ?f2 +?f0 -0
eq logheight/2
eq ?f4 +?f1 -0
eq ?f0 *logheight/logwidth
eq abs(?f1 )
eq abs(?f6 )
eq ?f7 +0-?f8
eq if(?f0 ,7,2)
eq if(?f0 ,10,5)
eq logwidth*?f10 /12
eq logwidth*?f11 /12
eq if(?f1 ,7,2)
eq if(?f1 ,10,5)
eq logheight*?f14 /12
eq logheight*?f15 /12
eq if(?f0 ,0,?f3 )
eq if(?f9 ,0,?f18 )
eq if(?f1 ,?f12 ,?f3 )
eq if(?f9 ,?f20 ,?f12 )
eq if(?f0 ,?f3 ,logwidth)
eq if(?f9 ,logwidth,?f22 )
eq if(?f1 ,?f3 ,?f12 )
eq if(?f9 ,?f24 ,?f12 )
eq if(?f0 ,?f16 ,?f5 )
eq if(?f9 ,?f16 ,?f26 )
eq if(?f1 ,0,?f5 )
eq if(?f9 ,?f28 ,0)
eq if(?f0 ,?f5 ,?f16 )
eq if(?f9 ,?f16 ,?f30 )
eq if(?f1 ,?f5 ,logheight)
eq if(?f9 ,?f32 ,logheight)
eq min(logwidth,logheight)
eq ?f34 *$2 /100000
eq logwidth+0-?f35
eq logheight+0-?f35
eq ?f35 *29289/100000
eq logwidth+0-?f38
eq logheight+0-?f38
eq logheight
eq logwidth
eq (10800000)/60000.0
eq (5400000)/60000.0
eq (16200000)/60000.0
eq (5400000)/60000.0
eq (0)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
eq (5400000)/60000.0
path M 0 ?f35 G ?f35 ?f35 ?f43 ?f44 L ?f12 0 L ?f21 ?f29 L ?f13 0 L ?f36 0 G ?f35 ?f35 ?f45 ?f46 L ?f42 ?f16 L ?f23 ?f31 L ?f42 ?f17 L ?f42 ?f37 G ?f35 ?f35 ?f47 ?f48 L ?f13 ?f41 L ?f25 ?f33 L ?f12 ?f41 L ?f35 ?f41 G ?f35 ?f35 ?f49 ?f50 L 0 ?f17 L ?f19 ?f27 L 0 ?f16 Z N
";
