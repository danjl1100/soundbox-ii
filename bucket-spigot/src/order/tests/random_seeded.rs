// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::NONEMPTY_WEIGHTS;
use crate::order::source::{OrderSource, Random, Shuffle};
use crate::tests::decode_hex;
use crate::Weights;
use arbtest::arbitrary::Unstructured;

macro_rules! fake_rng {
    (let ($rng:ident, $u:ident) = $determined:expr;) => {
        let mut $u = Unstructured::new($determined);
        let $rng = &mut crate::tests::fake_rng(&mut $u);
    };
    (let ($rng:ident) = $determined:expr;) => {
        fake_rng! {
            let ($rng, _u) = $determined;
        }
    };
}

fn fmt_chunks<R: rand::Rng + ?Sized>(
    uut: &mut impl OrderSource<R>,
    weights: Weights<'_>,
    rng: &mut R,
) -> String {
    use std::fmt::Write as _;
    let mut iter = std::iter::repeat_with(|| uut.next(rng, weights).expect("should not fail"));

    // NOTE: assert_debug_snapshot is too verbose (line endings from `:#?`)
    let mut output = String::new();
    for _line in 0..5 {
        for elem in 0..30 {
            let space = if elem == 0 { "" } else { " " };
            write!(&mut output, "{space}{}", iter.next().expect("infinite")).expect("infallible");
        }
        writeln!(&mut output).expect("infallible");
    }
    output
}
fn fmt_chunks_spatial<R: rand::Rng + ?Sized>(
    uut: &mut impl OrderSource<R>,
    weights: Weights<'_>,
    rng: &mut R,
    (len, repetitions): (usize, usize),
) -> String {
    use std::fmt::Write as _;
    let mut iter = std::iter::repeat_with(|| uut.next(rng, weights).expect("should not fail"));

    // NOTE: assert_debug_snapshot is too verbose (line endings from `:#?`)
    let mut output = String::new();
    for repetition in 0..repetitions {
        if repetition > 0 {
            writeln!(
                &mut output,
                "--------------------------------------------------------------------------------"
            )
            .expect("infallible");
        }
        for _elem in 0..len {
            let chosen = iter.next().expect("infinite");
            writeln!(
                &mut output,
                "{chosen: <5}\t{:<chosen$}{:<chosen$}><",
                "", ""
            )
            .expect("infallible");
        }
    }
    output
}
#[test]
fn random_looks_decent() {
    let determined = decode_hex(&[
        // chosen by fair dice roll, then paintsakingly trimmed to length
        // (`head --bytes=? /dev/urandom | sha256sum`)
        "650ef28d459cd670558cc769820c0e6b09d41388667068cb0c73390604149f68",
        "568d8549e361fe905d928bf3d0f862c046f5c4cdb5cad04f8ef8b956341483e6",
        "f2d88bf0fcf77f2a2f56ed81c94479e395133348f3958374050b9cc06eb5b129",
        "aa5b814f53abe186cfacbbfdabe2f90ab8c071dc4ed50dcf1d4362a46f0e6348",
        "0ada9fbdc9962e02271c7f93fa2cbe5389cdebf13e8f",
    ])
    .expect("valid hex strings");

    let weights = &[10, 2, 1]; // 0 - 2
    let weights = Weights::new_custom(weights).expect(NONEMPTY_WEIGHTS);

    let mut uut = Random::default();

    fake_rng! {
        let (rng, u) = &determined;
    }
    insta::assert_snapshot!(fmt_chunks(&mut uut, weights, rng), @r###"
    1 0 0 1 0 0 0 0 0 1 0 0 0 2 0 0 0 0 0 0 1 0 0 0 2 1 0 0 0 0
    0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 1 2 0 0 0 2 0 0 0
    0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 2 0 0 0 0 0 0 2 0 0 0 0 2 0 1
    0 1 0 2 0 0 0 0 2 0 0 0 0 0 2 0 0 0 0 0 0 1 0 1 0 2 0 0 0 2
    0 0 0 0 0 0 0 0 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 0 1 0
    "###);
    assert_eq!(u.len(), 0);

    let weights = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 2, 1]; // 10-12
    let weights = Weights::new_custom(weights).expect(NONEMPTY_WEIGHTS);

    let mut u = Unstructured::new(&determined);
    let rng = &mut crate::tests::fake_rng(&mut u);
    insta::assert_snapshot!(fmt_chunks(&mut uut, weights, rng), @r###"
    11 10 10 11 10 10 10 10 10 11 10 10 10 12 10 10 10 10 10 10 11 10 10 10 12 11 10 10 10 10
    10 10 10 11 10 10 10 10 10 10 10 10 10 10 10 10 10 11 10 11 10 11 12 10 10 10 12 10 10 10
    10 10 10 10 10 10 10 10 10 10 11 10 10 10 10 12 10 10 10 10 10 10 12 10 10 10 10 12 10 11
    10 11 10 12 10 10 10 10 12 10 10 10 10 10 12 10 10 10 10 10 10 11 10 11 10 12 10 10 10 12
    10 10 10 10 10 10 10 10 11 11 10 10 10 10 10 10 10 10 11 10 10 10 10 10 10 11 10 10 11 10
    "###);
    assert_eq!(u.len(), 0);
}
#[allow(clippy::too_many_lines)]
#[test]
fn shuffle_looks_decent() {
    let determined = decode_hex(&[
        // chosen by fair dice roll, then paintsakingly trimmed to length
        // (`head --bytes=? /dev/urandom | sha256sum`)
        "4bffec6f407ff21d7ec94baa6f39b3b78d52658f10d95f3aae50afe2295f18dc",
        "c92fc8641aa86e0a901ce582dc823caa5afbbe5ff70fb409575d",
    ])
    .expect("valid hex strings");

    let sum = |weights: &[u32]| -> usize { weights.iter().map(|&n| n as usize).sum() };

    {
        let mut uut = Shuffle::default();

        let weights = &[10, 2, 1]; // 0 - 2
        let weights_sum = sum(weights);
        let weights = Weights::new_custom(weights).expect(NONEMPTY_WEIGHTS);
        fake_rng! {
            let (rng) = &determined;
        }
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 2)), @r###"
        0    	><
        1    	  ><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        1    	  ><
        0    	><
        2    	    ><
        0    	><
        --------------------------------------------------------------------------------
        0    	><
        1    	  ><
        0    	><
        0    	><
        1    	  ><
        0    	><
        2    	    ><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        "###);
    }

    {
        let mut uut = Shuffle::default();

        let weights_sum = 5;
        let weights = Weights::new_equal(weights_sum).expect(NONEMPTY_WEIGHTS);
        fake_rng! {
            let (rng) = &determined;
        }
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 3)), @r###"
        1    	  ><
        2    	    ><
        0    	><
        4    	        ><
        3    	      ><
        --------------------------------------------------------------------------------
        4    	        ><
        0    	><
        3    	      ><
        2    	    ><
        1    	  ><
        --------------------------------------------------------------------------------
        2    	    ><
        3    	      ><
        1    	  ><
        0    	><
        4    	        ><
        "###);
    }

    {
        let mut uut = Shuffle::default();
        let weights_sum = 30;
        let weights = Weights::new_equal(weights_sum).expect(NONEMPTY_WEIGHTS);
        fake_rng! {
            let (rng, u) = &determined;
        }
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 2)), @r###"
        10   	                    ><
        23   	                                              ><
        0    	><
        26   	                                                    ><
        9    	                  ><
        7    	              ><
        28   	                                                        ><
        2    	    ><
        13   	                          ><
        11   	                      ><
        12   	                        ><
        25   	                                                  ><
        20   	                                        ><
        21   	                                          ><
        17   	                                  ><
        6    	            ><
        16   	                                ><
        14   	                            ><
        29   	                                                          ><
        22   	                                            ><
        4    	        ><
        24   	                                                ><
        27   	                                                      ><
        18   	                                    ><
        1    	  ><
        5    	          ><
        15   	                              ><
        8    	                ><
        19   	                                      ><
        3    	      ><
        --------------------------------------------------------------------------------
        18   	                                    ><
        13   	                          ><
        21   	                                          ><
        2    	    ><
        0    	><
        20   	                                        ><
        23   	                                              ><
        24   	                                                ><
        8    	                ><
        1    	  ><
        11   	                      ><
        26   	                                                    ><
        14   	                            ><
        16   	                                ><
        25   	                                                  ><
        7    	              ><
        10   	                    ><
        4    	        ><
        3    	      ><
        19   	                                      ><
        27   	                                                      ><
        9    	                  ><
        6    	            ><
        22   	                                            ><
        5    	          ><
        17   	                                  ><
        29   	                                                          ><
        15   	                              ><
        12   	                        ><
        28   	                                                        ><
        "###);

        // longest, ensure we used all entropy (to not specify more than needed)
        assert_eq!(u.len(), 0);
    }

    {
        let determined = decode_hex(&[
            // chosen by fair dice roll, then paintsakingly trimmed to length
            // (`head --bytes=? /dev/urandom | sha256sum`)
            "fd9dd4649d4c460cfa9ec3e3c6b3fc4e7708361b9a01a567493af2b6a1e8855b2888eb00622a517d573b900c662fa732a08a36ca924987721e540a69bb150ec6513bb7449632c928cd23d0836f410294fbb9b76dafcd57c30540412dfbb3dae9dc0bd5386008293f6f05d2bf843afde9e01fc418ec63d5bdd9fd91f3dc1680730246a2bf921a012fb1ca144c0b52d1f995af252b17375ba72d4fe3a019cbdc3110a9301b3a8f9e799ab13e47e63f185116b2eb5c3e8ff1b072833a236b27f3ff78e3bcea1d60792a6ee50d53d956945fc83995d8ab1499f368c17dce00949080366aaf38485b3deb09773a05fa9fc878be1fcdfe0f63030358f8f24c467f36aff622ff51b9a3fd29f16561c2352b7cc9ab1460d8a692b26f77843537c2fdbcf7fc6dd6a6c0bc63439720e8",
        ])
        .expect("valid hex strings");
        let mut uut = Shuffle::default();
        let weights_sum = 300;
        let weights = Weights::new_equal(weights_sum).expect(NONEMPTY_WEIGHTS);
        fake_rng! {
            let (rng, u) = &determined;
        }
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 1)));

        // longest, ensure we used all entropy (to not specify more than needed)
        assert_eq!(u.len(), 0);
    }
}
