// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! SVG-render related functions and data definitions
use yew::prelude::*;

macro_rules! svg_paths {
    (
        $struct_vis:vis struct $struct:ident { path, width, height }
        $(
            $item_vis:vis const $name:ident = {
                [ $width:expr , $height:expr $(,)?];
                $(
                    $cmd:tt $($arg:expr),*
                );+ $(;)?
            };
        )+
    ) => {
        $struct_vis struct $struct {
            path: &'static str,
            width: &'static str,
            height: &'static str,
            view_box: &'static str,
        }
        $(
            $item_vis const $name : &$struct = &$struct {
                path: concat!(
                    $(
                        svg_paths!(@path_d $cmd $($arg),*)
                    ),+
                ),
                width: stringify!($width),
                height: stringify!($height),
                view_box: svg_paths!(@str_cat 0, 0, $width, $height),
            };
        )+
    };
    (@path_d $cmd:tt $($arg:expr),*) => {
        concat!(
            svg_paths!(@valid_cmd $cmd $($arg),*),
            svg_paths!(@str_cat $cmd, $($arg),*)
        )
    };
    (@str_cat $($arg:tt),+ $(,)?) => {
        concat!( $( " ", stringify!($arg)),+ )
    };
    (@valid_cmd M $_x:expr, $_y:expr) => { "" };
    (@valid_cmd l $_x:expr, $_y:expr) => { "" };
    (@valid_cmd h $_x:expr) => { "" };
    (@valid_cmd v $_y:expr) => { "" };
    (@valid_cmd z) => { "" };
}

svg_paths! {
    pub(crate) struct Def { path, width, height }
    pub(crate) const PLAY = {
        [12, 12]; // 1-10, with extra margin
        // triangle (x,y = 1-10)
        M 1, 1;
        v 10;
        l 10, -5;
        z;
    };
    pub(crate) const PAUSE = {
        [10, 10]; // 1-8, with extra margin
        // left box (x = 1-4)
        M 1, 1;
        v 8;
        h 3;
        v -8;
        z;
        // right box (x = 6-9)
        M 6, 1;
        v 8;
        h 3;
        v -8;
        z;
    };
    pub(crate) const NEXT = {
        [16, 8]; // width 14, with extra margin
        // right-ward triangle 1
        M 1, 1;
        v 6;
        l 6, -3;
        z;
        // right-ward triangle 2
        M 7, 1;
        v 6;
        l 6, -3;
        z;
        // right-most bar
        M 13, 1;
        v 6;
        h 2;
        v -6;
        z;
    };
    pub(crate) const PREV = {
        [16, 8]; // width 14, with extra margin
        // left-ward triangle 1
        M 15, 1;
        v 6;
        l -6, -3;
        z;
        // left-ward triangle 2
        M 9, 1;
        v 6;
        l -6, -3;
        z;
        // left-most bar
        M 3, 1;
        v 6;
        h -2;
        v -6;
        z;
    };
    pub(crate) const FORWARD = {
        [14, 8]; // width 12, with extra margin
        // right-ward triangle 1
        M 1, 1;
        v 6;
        l 6, -3;
        z;
        // right-ward triangle 2
        M 7, 1;
        v 6;
        l 6, -3;
        z;
    };
    pub(crate) const BACKWARD = {
        [14, 8]; // width 12, with extra margin
        // left-ward triangle 1
        M 13, 1;
        v 6;
        l -6, -3;
        z;
        // left-ward triangle 2
        M 7, 1;
        v 6;
        l -6, -3;
        z;
    };
    pub(crate) const X_CROSS = {
        [8, 8];
        M 1, 2;
        l 5, 5;
        l 1, -1;
        l -5, -5;
        z;
        M 2, 7;
        l 5, -5;
        l -1, -1;
        l -5, 5;
        z;
    };
    pub(crate) const PLUS = {
        [8, 8];
        M 5, 1;
        v 6;
        h -2;
        v -6;
        z;
        M 1, 3;
        h 6;
        v 2;
        h -6;
        z;
    };
    pub(crate) const MINUS = {
        [8, 8];
        M 1, 3;
        h 6;
        v 2;
        h -6;
        z;
    };
    pub(crate) const EMPTY = {
        [1, 1];
        M 0, 0;
    };
}

pub(crate) struct Renderer {
    pub stroke: &'static str,
    pub fill: &'static str,
}
impl Renderer {
    pub fn render(&self, def: &Def) -> Html {
        html! {
            <svg viewBox=def.view_box>
                <path d=def.path stroke=self.stroke fill=self.fill />
            </svg>
        }
    }
}
