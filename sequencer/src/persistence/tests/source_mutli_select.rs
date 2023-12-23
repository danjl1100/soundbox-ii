// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use serde::{Deserialize, Serialize};

use crate::{
    persistence::{OptionStructSerializeDeserialize, SequencerConfig},
    source_multi_select,
    sources::{Beet, FileLines, FolderListing},
    DebugItemSource,
};

source_multi_select! {
    pub(crate) struct Source {
        type Args = Args<'a>;
        #[derive(Copy, Clone)]
        type Type = Type;
        /// Beet
        beet: Beet as Beet where arg type = Vec<String>,
        /// File lines
        file_lines: FileLines as FileLines where arg type = String,
        /// Folder listing
        folder_listing: FolderListing as FolderListing where arg type = String,
        /// Debug
        debug: DebugItemSource as Debug where arg type = String,
    }
    #[derive(Clone, Debug, Serialize, Deserialize)]
    /// Typed argument
    impl ItemSource<Option<TypedArg>> {
        type Item = String;
        /// Typed Error
        type Error = TypedLookupError;
    }
}
impl OptionStructSerializeDeserialize for TypedArg {}

const INPUT: &str = r#"/* comment */
root {
    leaf type="Beet" "arg1" "arg2";
    chain type="FileLines" "single-arg" {
    }
    leaf type="FolderListing" "single-arg-again";
    chain type="Debug" "this is debug" {
        chain type="Debug" "yet more debug" {
            leaf type="Debug" "hey, more debug!";
        }
    }
}
"#;

#[test]
fn round_trip_multi_select() {
    let (mut config, tree) =
        SequencerConfig::<String, Option<TypedArg>>::parse_from_str(INPUT).expect("valid KDL");

    let updated = config.update_to_string(&tree).expect("serialize again");
    assert_eq!(updated, INPUT);
}
