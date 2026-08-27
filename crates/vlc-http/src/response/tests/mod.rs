// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

#![allow(clippy::panic, reason = "expect panics in tests")]

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
            .nth(7)
            .expect("test exe within target/debug/build/vlc-http/????/out/vlc-http-????")
            .to_path_buf();
        // crate root
        path_buf.extend(&["crates", "vlc-http", "src", "response", "tests", "input"]);
        path_buf
    };

    assert!(
        input_folder.exists(),
        "input folder not found: {}",
        input_folder.display()
    );

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
