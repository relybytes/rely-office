// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The preset shapes of `DrawingML`, in the guide language of ECMA-376 20.1.9 written one
//! statement to a line. A preset missing here is looked up in `enhanced_presets.rs`.

pub(super) const PRESETS: &str = r"
[rect]
M l t
L r t
L r b
L l b
Z

[roundRect]
av adj 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
M l x1
A x1 x1 cd2 cd4
L x2 t
A x1 x1 3cd4 cd4
L r y2
A x1 x1 0 cd4
L x1 b
A x1 x1 cd4 cd4
Z

[triangle]
av adj 50000
gd a pin 0 adj 100000
gd x1 */ w a 100000
M l b
L x1 t
L r b
Z

[rtTriangle]
M l t
L r b
L l b
Z

[diamond]
M l vc
L hc t
L r vc
L hc b
Z

[parallelogram]
av adj 25000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x2 */ ss a 100000
gd x5 +- r 0 x2
M l b
L x2 t
L r t
L x5 b
Z

[trapezoid]
av adj 25000
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd x2 */ ss a 100000
gd x3 +- r 0 x2
M l b
L x2 t
L x3 t
L r b
Z

[pentagon]
av hf 105146
av vf 110557
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc vf 100000
gd dx1 cos swd2 1080000
gd dx2 cos swd2 18360000
gd dy1 sin shd2 1080000
gd dy2 sin shd2 18360000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- svc 0 dy1
gd y2 +- svc 0 dy2
M x1 y1
L hc t
L x4 y1
L x3 y2
L x2 y2
Z

[hexagon]
av adj 25000
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 100000
gd x2 +- r 0 x1
M l vc
L x1 t
L x2 t
L r vc
L x2 b
L x1 b
Z

[heptagon]
regular 7

[octagon]
av adj 29289
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
M l x1
L x1 t
L x2 t
L r x1
L r y2
L x2 b
L x1 b
L l y2
Z

[decagon]
regular 10

[dodecagon]
regular 12

[star4]
av adj 12500
star 4

[star5]
av adj 19098
star 5

[star6]
av adj 28868
star 6

[star7]
av adj 34601
star 7

[star8]
av adj 38250
star 8

[star10]
av adj 42533
star 10

[star12]
av adj 37500
star 12

[star16]
av adj 37500
star 16

[star24]
av adj 37500
star 24

[star32]
av adj 37500
star 32

[irregularSeal1]
av adj 30000
star 12

[irregularSeal2]
av adj 34000
star 14

[rightArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx1 */ ss a2 100000
gd x1 +- r 0 dx1
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
M l y1
L x1 y1
L x1 t
L r vc
L x1 b
L x1 y2
L l y2
Z

[leftArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd x2 */ ss a2 100000
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
M l vc
L x2 t
L x2 y1
L r y1
L r y2
L x2 y2
L x2 b
Z

[upArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd y2 */ ss a2 100000
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
M l y2
L hc t
L r y2
L x2 y2
L x2 b
L x1 b
L x1 y2
Z

[downArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dy1 */ ss a2 100000
gd y1 +- b 0 dy1
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
M l y1
L x1 y1
L x1 t
L x2 t
L x2 y1
L r y1
L hc b
Z

[leftRightArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 50000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd x2 */ ss a2 100000
gd x3 +- r 0 x2
gd dy */ h a1 200000
gd y1 +- vc 0 dy
gd y2 +- vc dy 0
M l vc
L x2 t
L x2 y1
L x3 y1
L x3 t
L r vc
L x3 b
L x3 y2
L x2 y2
L x2 b
Z

[upDownArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 50000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd y2 */ ss a2 100000
gd y3 +- b 0 y2
gd dx */ w a1 200000
gd x1 +- hc 0 dx
gd x2 +- hc dx 0
M l y2
L hc t
L r y2
L x2 y2
L x2 y3
L r y3
L hc b
L l y3
L x1 y3
L x1 y2
Z

[quadArrow]
av adj1 22500
av adj2 22500
av adj3 22500
gd a2 pin 0 adj2 50000
gd maxAdj1 */ a2 2 1
gd a1 pin 0 adj1 maxAdj1
gd q1 +- 100000 0 maxAdj1
gd maxAdj3 */ q1 1 2
gd a3 pin 0 adj3 maxAdj3
gd x1 */ ss a3 100000
gd dx2 */ ss a2 100000
gd x2 +- hc 0 dx2
gd x5 +- hc dx2 0
gd dx3 */ ss a1 200000
gd x3 +- hc 0 dx3
gd x4 +- hc dx3 0
gd x6 +- r 0 x1
gd y2 +- vc 0 dx2
gd y5 +- vc dx2 0
gd y3 +- vc 0 dx3
gd y4 +- vc dx3 0
gd y6 +- b 0 x1
M l vc
L x1 y2
L x1 y3
L x3 y3
L x3 x1
L x2 x1
L hc t
L x5 x1
L x4 x1
L x4 y3
L x6 y3
L x6 y2
L r vc
L x6 y5
L x6 y4
L x4 y4
L x4 y6
L x5 y6
L hc b
L x2 y6
L x3 y6
L x3 y4
L x1 y4
L x1 y5
Z

[notchedRightArrow]
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx2 */ ss a2 100000
gd x2 +- r 0 dx2
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd x1 */ dy1 dx2 hd2
M l y1
L x2 y1
L x2 t
L r vc
L x2 b
L x2 y2
L l y2
L x1 vc
Z


[chevron]
av adj 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 100000
gd x2 +- r 0 x1
M l t
L x2 t
L r vc
L x2 b
L l b
L x1 vc
Z

[homePlate]
av adj 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
M l t
L x1 t
L r vc
L x1 b
L l b
Z

[plus]
av adj 25000
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
M l x1
L x1 x1
L x1 t
L x2 t
L x2 x1
L r x1
L r y2
L x2 y2
L x2 b
L x1 b
L x1 y2
L l y2
Z

[mathPlus]
av adj1 23520
gd a1 pin 0 adj1 73490
gd dx1 */ w 73490 200000
gd dy1 */ h 73490 200000
gd dx2 */ ss a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- vc 0 dy1
gd y2 +- vc 0 dx2
gd y3 +- vc dx2 0
gd y4 +- vc dy1 0
M x1 y2
L x2 y2
L x2 y1
L x3 y1
L x3 y2
L x4 y2
L x4 y3
L x3 y3
L x3 y4
L x2 y4
L x2 y3
L x1 y3
Z

[mathMinus]
av adj1 23520
gd a1 pin 0 adj1 100000
gd dy1 */ h a1 200000
gd dx1 */ w 73490 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
M x1 y1
L x2 y1
L x2 y2
L x1 y2
Z

[mathEqual]
av adj1 23520
av adj2 11760
gd a1 pin 0 adj1 36745
gd a2 pin 0 adj2 50000
gd dy1 */ h a1 100000
gd dy2 */ h a2 200000
gd dx1 */ w 73490 200000
gd y2 +- vc 0 dy2
gd y3 +- vc dy2 0
gd y1 +- y2 0 dy1
gd y4 +- y3 dy1 0
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
M x1 y1
L x2 y1
L x2 y2
L x1 y2
Z
M x1 y3
L x2 y3
L x2 y4
L x1 y4
Z

[mathMultiply]
av adj1 23520
gd a1 pin 0 adj1 51965
gd th */ ss a1 141421
gd m */ ss 1 10
gd x1 +- m th 0
gd x2 +- r 0 m
gd x3 +- x2 0 th
gd y1 +- m th 0
gd y2 +- b 0 m
gd y3 +- y2 0 th
gd yc1 +- vc 0 th
gd yc2 +- vc th 0
gd xc1 +- hc 0 th
gd xc2 +- hc th 0
M m y1
L x1 m
L hc yc1
L x3 m
L x2 y1
L xc2 vc
L x2 y3
L x3 y2
L hc yc2
L x1 y2
L m y3
L xc1 vc
Z

[wedgeRectCallout]
av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd dq */ dxPos h w
gd ady abs dyPos
gd adq abs dq
gd dz +- ady 0 adq
gd xg1 ?: dxPos 7 2
gd xg2 ?: dxPos 10 5
gd x1 */ w xg1 12
gd x2 */ w xg2 12
gd yg1 ?: dyPos 7 2
gd yg2 ?: dyPos 10 5
gd y1 */ h yg1 12
gd y2 */ h yg2 12
gd t1 ?: dxPos l xPos
gd xl ?: dz l t1
gd t2 ?: dyPos x1 xPos
gd xt ?: dz t2 x1
gd t3 ?: dxPos xPos r
gd xr ?: dz r t3
gd t4 ?: dyPos xPos x1
gd xb ?: dz t4 x1
gd t5 ?: dxPos y1 yPos
gd yl ?: dz y1 t5
gd t6 ?: dyPos t yPos
gd yt ?: dz t6 t
gd t7 ?: dxPos yPos y1
gd yr ?: dz y1 t7
gd t8 ?: dyPos yPos b
gd yb ?: dz t8 b
M l t
L x1 t
L xt yt
L x2 t
L r t
L r y1
L xr yr
L r y2
L r b
L x2 b
L xb yb
L x1 b
L l b
L l y2
L xl yl
L l y1
Z


[wedgeEllipseCallout]
av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd sdx */ dxPos h 1
gd sdy */ dyPos w 1
gd pang at2 sdx sdy
gd stAng +- pang 660000 0
gd enAng +- pang 0 660000
gd dx1 cos wd2 stAng
gd dy1 sin hd2 stAng
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd dx2 cos wd2 enAng
gd dy2 sin hd2 enAng
gd stAng1 at2 dx1 dy1
gd enAng1 at2 dx2 dy2
gd sw1 +- enAng1 0 stAng1
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
M xPos yPos
L x1 y1
A wd2 hd2 stAng1 swAng
Z


[borderCallout1]
av adj1 18750
av adj2 -8333
av adj3 112500
av adj4 -38333
gd y1 */ h adj1 100000
gd x1 */ w adj2 100000
gd y2 */ h adj3 100000
gd x2 */ w adj4 100000
M l t
L r t
L r b
L l b
Z
path fill=none
M x1 y1
L x2 y2


[borderCallout2]
av adj1 18750
av adj2 -8333
av adj3 18750
av adj4 -16667
av adj5 112500
av adj6 -46667
gd y1 */ h adj1 100000
gd x1 */ w adj2 100000
gd y2 */ h adj3 100000
gd x2 */ w adj4 100000
gd y3 */ h adj5 100000
gd x3 */ w adj6 100000
M l t
L r t
L r b
L l b
Z
path fill=none
M x1 y1
L x2 y2
L x3 y3


[heart]
gd dx1 */ w 49 48
gd dx2 */ w 10 48
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- t 0 hd3
M hc hd4
C x3 y1 x4 hd4 hc b
C x1 hd4 x2 y1 hc hd4
Z

[lightningBolt]
path w=21600 h=21600
M 8458 0
L 12960 6923
L 11187 7564
L 16994 12575
L 15232 13230
L 21600 21600
L 10014 14915
L 12169 14072
L 5022 9230
L 7343 8331
L 0 3594
Z

[moon]
av adj 50000
gd a pin 0 adj 87500
gd g0 */ ss a 100000
gd iw +- w 0 g0
M r t
A w hd2 3cd4 -10800000
A iw hd2 cd4 10800000
Z

[donut]
av adj 25000
gd a pin 0 adj 50000
gd dr */ ss a 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
M l vc
A wd2 hd2 cd2 21600000
Z
M dr vc
A iwd2 ihd2 cd2 -21600000
Z

[frame]
av adj1 12500
gd a1 pin 0 adj1 50000
gd x1 */ ss a1 100000
gd x4 +- r 0 x1
gd y4 +- b 0 x1
M l t
L r t
L r b
L l b
Z
M x1 x1
L x1 y4
L x4 y4
L x4 x1
Z

[halfFrame]
av adj1 33333
av adj2 33333
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd x1 */ ss a2 100000
gd y1 */ ss a1 100000
gd x2 +- r 0 y1
gd y2 +- b 0 x1
M l t
L r t
L x2 y1
L x1 y1
L x1 y2
L l b
Z

[corner]
av adj1 50000
av adj2 50000
gd maxAdj1 */ 100000 h ss
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 maxAdj1
gd a2 pin 0 adj2 maxAdj2
gd x1 */ ss a2 100000
gd dy1 */ ss a1 100000
gd y1 +- b 0 dy1
M l t
L x1 t
L x1 y1
L r y1
L r b
L l b
Z

[plaque]
av adj 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
M l x1
A x1 x1 cd4 -5400000
L x2 t
A x1 x1 cd2 -5400000
L r y2
A x1 x1 3cd4 -5400000
L x1 b
A x1 x1 0 -5400000
Z

[snip1Rect]
av adj 16667
gd a pin 0 adj 50000
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
M l t
L x1 t
L r dx1
L r b
L l b
Z

[snip2SameRect]
av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
M tx1 t
L tx2 t
L r tx1
L r by1
L bx2 b
L bx1 b
L l by1
L l tx1
Z

[snip2DiagRect]
av adj1 0
av adj2 16667
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd lx1 */ ss a1 100000
gd lx2 +- r 0 lx1
gd ly1 +- b 0 lx1
gd rx1 */ ss a2 100000
gd rx2 +- r 0 rx1
gd ry1 +- b 0 rx1
M lx1 t
L rx2 t
L r rx1
L r ly1
L lx2 b
L rx1 b
L l ry1
L l lx1
Z

[snipRoundRect]
av adj1 16667
av adj2 16667
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 100000
gd x1 */ ss a1 100000
gd dx2 */ ss a2 100000
gd x2 +- r 0 dx2
M x1 t
L x2 t
L r dx2
L r b
L l b
L l x1
A x1 x1 cd2 cd4
Z

[round1Rect]
av adj 16667
gd a pin 0 adj 50000
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
M l t
L x1 t
A dx1 dx1 3cd4 cd4
L r b
L l b
Z

[round2SameRect]
av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
M tx1 t
L tx2 t
A tx1 tx1 3cd4 cd4
L r by1
A bx1 bx1 0 cd4
L bx1 b
A bx1 bx1 cd4 cd4
L l tx1
A tx1 tx1 cd2 cd4
Z

[round2DiagRect]
av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd x1 */ ss a1 100000
gd y1 +- b 0 x1
gd a */ ss a2 100000
gd x2 +- r 0 a
gd y2 +- b 0 a
M x1 t
L x2 t
A a a 3cd4 cd4
L r y1
A x1 x1 0 cd4
L a b
A a a cd4 cd4
L l x1
A x1 x1 cd2 cd4
Z

[teardrop]
av adj 100000
gd a pin 0 adj 200000
gd dx */ wd2 a 100000
gd dy */ hd2 a 100000
gd tx +- hc dx 0
gd ty +- vc 0 dy
M l vc
A wd2 hd2 cd2 cd4
L tx ty
L r vc
A wd2 hd2 0 cd2
Z

[pie]
av adj1 0
av adj2 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw11 +- enAng 0 stAng
gd sw12 +- sw11 21600000 0
gd swAng ?: sw11 sw11 sw12
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
M x1 y1
A wd2 hd2 stAng swAng
L hc vc
Z

[chord]
av adj1 2700000
av adj2 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
M x1 y1
A wd2 hd2 stAng swAng
Z

[arc]
av adj1 16200000
av adj2 0
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw11 +- enAng 0 stAng
gd sw12 +- sw11 21600000 0
gd swAng ?: sw11 sw11 sw12
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path fill=none
M x1 y1
A wd2 hd2 stAng swAng

[blockArc]
av adj1 10800000
av adj2 0
av adj3 25000
gd stAng pin 0 adj1 21599999
gd istAng pin 0 adj2 21599999
gd a3 pin 0 adj3 50000
gd sw11 +- istAng 0 stAng
gd sw12 +- sw11 21600000 0
gd swAng ?: sw11 sw11 sw12
gd iswAng +- 0 0 swAng
gd dr */ ss a3 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd wt3 sin iwd2 istAng
gd ht3 cos ihd2 istAng
gd dx3 cat2 iwd2 ht3 wt3
gd dy3 sat2 ihd2 ht3 wt3
gd x3 +- hc dx3 0
gd y3 +- vc dy3 0
M x1 y1
A wd2 hd2 stAng swAng
L x3 y3
A iwd2 ihd2 istAng iswAng
Z

[leftBracket]
av adj 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
path fill=none
M r b
A w y1 cd4 cd4
L l y1
A w y1 cd2 cd4

[rightBracket]
av adj 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
path fill=none
M l t
A w y1 3cd4 cd4
L r y2
A w y1 0 cd4

[bracketPair]
av adj 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
path fill=none
M x1 b
A x1 x1 cd4 cd4
L l x1
A x1 x1 cd2 cd4
M x2 t
A x1 x1 3cd4 cd4
L r y2
A x1 x1 0 cd4

[flowChartProcess=rect]
[flowChartAlternateProcess=roundRect]
[flowChartDecision=diamond]
[flowChartExtract=triangle]
[flowChartConnector]
M l vc
A wd2 hd2 cd2 21600000
Z

[flowChartInputOutput]
gd x2 */ w 1 5
gd x5 */ w 4 5
M l b
L x2 t
L r t
L x5 b
Z

[flowChartPredefinedProcess]
gd x1 */ w 1 8
gd x2 */ w 7 8
M l t
L r t
L r b
L l b
Z
path fill=none
M x1 t
L x1 b
M x2 t
L x2 b

[flowChartInternalStorage]
gd x1 */ w 1 8
gd y1 */ h 1 8
M l t
L r t
L r b
L l b
Z
path fill=none
M x1 t
L x1 b
M l y1
L r y1

[flowChartDocument]
path w=21600 h=21600
M 0 0
L 21600 0
L 21600 17322
C 10800 17322 10800 23922 0 20172
Z


[flowChartTerminator]
path w=21600 h=21600
M 3475 0
L 18125 0
A 3475 10800 3cd4 cd2
L 3475 21600
A 3475 10800 cd4 cd2
Z

[flowChartPreparation]
gd x2 */ w 1 5
gd x3 */ w 4 5
M l vc
L x2 t
L x3 t
L r vc
L x3 b
L x2 b
Z

[flowChartManualInput]
gd y1 */ h 1 5
M l y1
L r t
L r b
L l b
Z

[flowChartManualOperation]
gd x1 */ w 1 5
gd x2 */ w 4 5
M l t
L r t
L x2 b
L x1 b
Z

[flowChartOffpageConnector]
gd y1 */ h 4 5
M l t
L r t
L r y1
L hc b
L l y1
Z

[flowChartPunchedCard]
gd x1 */ w 1 5
gd y1 */ h 1 5
M l y1
L x1 t
L r t
L r b
L l b
Z

[flowChartMerge]
M l t
L r t
L hc b
Z

[flowChartCollate]
M l t
L r t
L l b
L r b
Z

[flowChartSort]
M l vc
L hc t
L r vc
L hc b
Z
path fill=none
M l vc
L r vc

[flowChartSummingJunction]
gd dx cos wd2 2700000
gd dy sin hd2 2700000
gd x1 +- hc 0 dx
gd x2 +- hc dx 0
gd y1 +- vc 0 dy
gd y2 +- vc dy 0
M l vc
A wd2 hd2 cd2 21600000
Z
path fill=none
M x1 y1
L x2 y2
M x2 y1
L x1 y2

[flowChartOr]
M l vc
A wd2 hd2 cd2 21600000
Z
path fill=none
M hc t
L hc b
M l vc
L r vc

[flowChartDelay]
M l t
L hc t
A wd2 hd2 3cd4 cd2
L l b
Z

[flowChartDisplay]
gd x1 */ w 1 6
gd x2 */ w 5 6
M l vc
L x1 t
L x2 t
A x1 hd2 3cd4 cd2
L x1 b
Z

[flowChartOnlineStorage]
gd x1 */ w 1 6
gd x2 */ w 5 6
M x1 t
L r t
A x1 hd2 3cd4 -10800000
L x1 b
A x1 hd2 cd4 cd2
Z
";
