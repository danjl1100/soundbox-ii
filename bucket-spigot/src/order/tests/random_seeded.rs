// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! NOTE: "fair dice roll" is an homage to <https://xkcd.com/221/>

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
) -> Result<String, rand::Error> {
    use std::fmt::Write as _;
    let mut iter = std::iter::repeat_with(|| uut.next(rng, weights));

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
            let chosen = iter.next().expect("infinite iterator")?;
            writeln!(
                &mut output,
                "{chosen: <5}\t{:<chosen$}{:<chosen$}><",
                "", ""
            )
            .expect("infallible");
        }
    }
    Ok(output)
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
fn shuffle_looks_decent() -> eyre::Result<()> {
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
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 2))?, @r###"
        1    	  ><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        0    	><
        2    	    ><
        0    	><
        0    	><
        0    	><
        0    	><
        1    	  ><
        --------------------------------------------------------------------------------
        0    	><
        0    	><
        0    	><
        1    	  ><
        0    	><
        0    	><
        1    	  ><
        0    	><
        0    	><
        0    	><
        2    	    ><
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
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 3))?, @r###"
        0    	><
        3    	      ><
        2    	    ><
        1    	  ><
        4    	        ><
        --------------------------------------------------------------------------------
        4    	        ><
        3    	      ><
        2    	    ><
        1    	  ><
        0    	><
        --------------------------------------------------------------------------------
        1    	  ><
        4    	        ><
        0    	><
        2    	    ><
        3    	      ><
        "###);
    }

    {
        let mut uut = Shuffle::default();
        let weights_sum = 30;
        let weights = Weights::new_equal(weights_sum).expect(NONEMPTY_WEIGHTS);
        fake_rng! {
            let (rng, u) = &determined;
        }
        insta::assert_snapshot!(fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 2))?, @r###"
        15   	                              ><
        23   	                                              ><
        12   	                        ><
        3    	      ><
        27   	                                                      ><
        2    	    ><
        24   	                                                ><
        6    	            ><
        16   	                                ><
        25   	                                                  ><
        29   	                                                          ><
        18   	                                    ><
        26   	                                                    ><
        22   	                                            ><
        17   	                                  ><
        19   	                                      ><
        1    	  ><
        4    	        ><
        5    	          ><
        0    	><
        21   	                                          ><
        13   	                          ><
        7    	              ><
        28   	                                                        ><
        10   	                    ><
        11   	                      ><
        14   	                            ><
        8    	                ><
        9    	                  ><
        20   	                                        ><
        --------------------------------------------------------------------------------
        5    	          ><
        24   	                                                ><
        28   	                                                        ><
        12   	                        ><
        21   	                                          ><
        0    	><
        4    	        ><
        3    	      ><
        14   	                            ><
        29   	                                                          ><
        10   	                    ><
        11   	                      ><
        19   	                                      ><
        8    	                ><
        2    	    ><
        17   	                                  ><
        23   	                                              ><
        16   	                                ><
        15   	                              ><
        18   	                                    ><
        1    	  ><
        9    	                  ><
        7    	              ><
        25   	                                                  ><
        22   	                                            ><
        27   	                                                      ><
        26   	                                                    ><
        13   	                          ><
        20   	                                        ><
        6    	            ><
        "###);

        // longest, ensure we used all entropy (to not specify more than needed)
        assert_eq!(u.len(), 0);
    }

    Ok(())
}

fn calculate_shuffle_equal(
    length: usize,
    determined: &[&str],
) -> Result<String, CalculateShuffleError> {
    let determined = decode_hex(determined).expect("determined should be valid hex strings");
    let mut uut = Shuffle::default();
    let weights_sum = length;
    let weights = Weights::new_equal(weights_sum).expect(NONEMPTY_WEIGHTS);
    fake_rng! {
        let (rng, u) = &determined;
    }
    let result = fmt_chunks_spatial(&mut uut, weights, rng, (weights_sum, 1))
        .map_err(CalculateShuffleError::Rand)?;

    // ensure we used all entropy (to not specify more than needed)
    let len = u.len();
    if len > 0 {
        return Err(CalculateShuffleError::ExcessEntropy { len });
    }

    Ok(result)
}
#[derive(Debug)]
enum CalculateShuffleError {
    Rand(rand::Error),
    ExcessEntropy { len: usize },
}
impl std::error::Error for CalculateShuffleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CalculateShuffleError::Rand(error) => Some(error),
            CalculateShuffleError::ExcessEntropy { len: _ } => None,
        }
    }
}
impl std::fmt::Display for CalculateShuffleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalculateShuffleError::Rand(_) => write!(f, "rand::Error failure reported"),
            CalculateShuffleError::ExcessEntropy { len } => {
                write!(f, "{len} bytes of excess entropy created")
            }
        }?;
        write!(f, " in calculate_shuffle_equal")
    }
}

#[test]
fn shuffle_for_300_items() -> eyre::Result<()> {
    let result = calculate_shuffle_equal(
        300,
        &[
            // chosen by fair dice roll, then paintsakingly trimmed to length
            // (`head --bytes=? /dev/urandom | sha256sum`)
            "fd9dd4649d4c460cfa9ec3e3c6b3fc4e7708361b9a01a567493af2b6a1e8855b",
            "2888eb00622a517d573b900c662fa732a08a36ca924987721e540a69bb150ec6",
            "513bb7449632c928cd23d0836f410294fbb9b76dafcd57c30540412dfbb3dae9",
            "dc0bd5386008293f6f05d2bf843afde9e01fc418ec63d5bdd9fd91f3dc168073",
            "0246a2bf921a012fb1ca144c0b52d1f995af252b17375ba72d4fe3a019cbdc31",
            "10a9301b3a8f9e799ab13e47e63f185116b2eb5c3e8ff1b072833a236b27f3ff",
            "78e3bcea1d60792a6ee50d53d956945fc83995d8ab1499f368c17dce00949080",
            "366aaf38485b3deb09773a05fa9fc878be1fcdfe0f63030358f8f24c467f36af",
            "f622ff51b9a3fd29f16561c2352b7cc9ab1460d8a692b26f77843537c2fdbcf7",
            "fc6dd6a6c0bc63439720e8734274e5158237c605531ed246383ad84d957d7efd",
            "73812aecc4107213344f20b00fb8bf5b52ea2fc2e9f275",
        ],
    )?;
    insta::assert_snapshot!(result);
    Ok(())
}

#[test]
fn shuffle_for_500_items() -> eyre::Result<()> {
    let result = calculate_shuffle_equal(
        500,
        &[
            // chosen by fair dice roll, then paintsakingly trimmed to length
            // (`head --bytes=? /dev/urandom | sha256sum`)
            "06b3bdbe82c88f8f10b88ebdb61a09190704eaa861ae302f446ba311cfbbc67f",
            "9b3e334dc065215bcb3478a2fc613ef2371b37be8c4ee26d187078b602fbc164",
            "463ae40d8623a31cc8739f470273ef004f656e70a216a877dd7bd5c6d600dd31",
            "c7c048451878c528fad413a38888d56c43e72c528fafde7fa9029f1e978f23ed",
            "6de0cc4c30e34a542d6be961803239f016152b5ea90c08096605ee6430aa6c2f",
            "87212d817726e6366056590315569af001d361d333c4f945c1e3fe789b400d84",
            "148886c45d5f788a8490e636ea01f607d6df13fb02a1cb956b3f1c40c7b62aa4",
            "668b902cb07ccbd4a771bdb45200142c1fbdb1cec24e9f230c0b57d7c89c9fc8",
            "f8d0b8bc28765694fd48298a8f34ceda70c47a209f8e280e86cc24728b7c2982",
            "addb61a0fd9d3c0d9c398d2f3ad2c7aa93e79537299dbd121979408512ab662f",
            "c1625ae4509eca88bf70f17ac49cd7d26db2c41b3990fd0d1f34e856f77dc6a8",
            "e6b182daf1193547a189b8c3d0e22c9bba81f18478648c5ccb2da493b2b8cfa4",
            "6d8d18f7acc904539b3818274db95b76b328c1eba8dc74a42f6f0fa57c05a45a",
            "4873f692c6c5122af31dc8b70bbf2b5d6acf2c007d70b2245bc7f5459f8df70d",
            "3bc7d323ed8e07a294c943f75f1cd2ae0e80b73195f9083f652a9d5db68c5f3f",
            "86045a6491802ca623a95de8d3a30d873d288ae3ef294046d49fd38f92e5b257",
            "afb8088ab96087313ba159a9f758186a35543c96377320f2a5322e33d0765b2f",
            "db99afd926cace25ae3137214e08c6b5efcd765d9f760dbce8de2397731a902a",
            "549198b58276d53a64701b385825f3c0d3e217b6c030b400b2c4451e863b1251",
            "d9b2003f7cb330fb63a5fbed5fd393b8a8e06277a7e5e300bbf7be8f6d8f840b",
            "1b5d6eb125895d892f3ef0ac21e9981cb984d41453eb6e6b42995d17aa85224d",
            "11fa9b9f8187b19444411edd341e22bade10a7d297a31eaceed295fd842e385d",
            "14d4a0cdf4e25fb532ac4d1165d8c32262a050f437cfee12491a53d964514aa1",
            "98e20a72c8bf7c",
        ],
    )?;
    insta::assert_snapshot!(result);
    Ok(())
}
