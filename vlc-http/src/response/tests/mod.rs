// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

#![allow(clippy::panic)] // that's what tests are supposed to do!

use crate::Response;
use std::{
    io::BufReader,
    path::{Path, PathBuf},
};
use test_log::test;
use tracing::{error, info};

#[test]
fn parse() -> Result<(), Box<dyn std::error::Error>> {
    let input_folder = {
        let mut path_buf: PathBuf = std::env::current_exe()?
            .ancestors()
            .nth(4)
            .expect("test exe within target/debug/deps/vlc_http-????")
            .to_path_buf();
        // crate root
        path_buf.extend(&["vlc-http", "src", "response", "tests", "input"]);
        path_buf
    };

    for entry in std::fs::read_dir(input_folder)? {
        let entry = entry?;
        if entry.metadata()?.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let path = entry.path();

            info!(%name, path=%path.display(), "test input");

            parse_file(&name, &path)?;
        } else {
            error!(path=%entry.path().display(), "invalid file type");
            panic!("invalid file type for {}", entry.path().display());
        }
    }
    Ok(())
}

fn parse_file(name: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let response = Response::from_reader(reader)?;
    insta::assert_ron_snapshot!(name, response);

    Ok(())
}
