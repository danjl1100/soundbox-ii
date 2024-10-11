// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{clap::ArgBounds, path::Path, ModifyCmd, Network};

impl<T, U> Network<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    fn arbitrary_typed<S>(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self>
    where
        S: Into<seed::Full<T, U>> + for<'a> arbitrary::Arbitrary<'a>,
    {
        let generator: NetworkGenerator<S, _, _> = u.arbitrary()?;
        Ok(generator.finish())
    }
    pub(crate) fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self>
    where
        T: for<'a> arbitrary::Arbitrary<'a>,
        U: for<'a> arbitrary::Arbitrary<'a>,
    {
        Self::arbitrary_typed::<seed::Full<_, _>>(u)
    }
}
impl<U> Network<never::Arg, U>
where
    U: ArgBounds + for<'a> arbitrary::Arbitrary<'a>,
{
    pub(crate) fn arbitrary_no_items(
        u: &mut arbitrary::Unstructured<'_>,
    ) -> arbitrary::Result<Self> {
        Self::arbitrary_typed::<seed::NoItems<_>>(u)
    }
}

mod seed {
    use super::never;
    use crate::{order::OrderType, path::Path, ModifyCmd};

    #[derive(arbtest::arbitrary::Arbitrary)]
    pub(super) enum OrderTypeSeed {
        InOrder,
        Random,
        Shuffle,
    }
    // Prove completeness of `OrderTypeSeed`
    impl From<OrderType> for OrderTypeSeed {
        fn from(value: OrderType) -> Self {
            use OrderType as Other;
            match value {
                Other::InOrder => Self::InOrder,
                Other::Random => Self::Random,
                Other::Shuffle => Self::Shuffle,
            }
        }
    }
    impl From<OrderTypeSeed> for OrderType {
        fn from(value: OrderTypeSeed) -> Self {
            use OrderTypeSeed as Other;
            match value {
                Other::InOrder => Self::InOrder,
                Other::Random => Self::Random,
                Other::Shuffle => Self::Shuffle,
            }
        }
    }
    #[derive(arbtest::arbitrary::Arbitrary)]
    pub(super) enum Full<T, U> {
        AddBucket,
        AddJoint,
        FillBucket { new_contents: Vec<T> },
        SetFilters { new_filters: Vec<U> },
        SetWeight { new_weight: u32 },
        SetOrderType { new_order_type: OrderTypeSeed },
    }
    // Prove completeness of `Full`
    impl<T, U> From<ModifyCmd<T, U>> for (Path, Full<T, U>) {
        fn from(value: ModifyCmd<T, U>) -> Self {
            use Full as Seed;
            use ModifyCmd as Cmd;
            match value {
                Cmd::AddBucket { parent } => (parent, Seed::AddBucket),
                Cmd::AddJoint { parent } => (parent, Seed::AddJoint),
                // NOTE: exclude `DeleteEmpty` when constructing networks, since it requires cumbersome bookkeeping
                Cmd::DeleteEmpty { path: _ } => unimplemented!("no path to delete"),
                Cmd::FillBucket {
                    bucket,
                    new_contents,
                } => (bucket, Seed::FillBucket { new_contents }),
                Cmd::SetFilters { path, new_filters } => (path, Seed::SetFilters { new_filters }),
                Cmd::SetWeight { path, new_weight } => (path, Seed::SetWeight { new_weight }),
                Cmd::SetOrderType {
                    path,
                    new_order_type,
                } => (
                    path,
                    Seed::SetOrderType {
                        new_order_type: new_order_type.into(),
                    },
                ),
            }
        }
    }
    impl<T, U> From<(Path, Full<T, U>)> for ModifyCmd<T, U> {
        fn from(value: (Path, Full<T, U>)) -> Self {
            use Full as Seed;
            use ModifyCmd as Cmd;
            match value {
                (parent, Seed::AddBucket) => Cmd::AddBucket { parent },
                (parent, Seed::AddJoint) => Cmd::AddJoint { parent },
                (bucket, Seed::FillBucket { new_contents }) => Cmd::FillBucket {
                    bucket,
                    new_contents,
                },
                (path, Seed::SetFilters { new_filters }) => Cmd::SetFilters { path, new_filters },
                (path, Seed::SetWeight { new_weight }) => Cmd::SetWeight { path, new_weight },
                (path, Seed::SetOrderType { new_order_type }) => Cmd::SetOrderType {
                    path,
                    new_order_type: new_order_type.into(),
                },
            }
        }
    }

    #[derive(arbtest::arbitrary::Arbitrary)]
    pub(super) enum NoItems<U> {
        AddBucket,
        AddJoint,
        SetFilters { new_filters: Vec<U> },
        SetWeight { new_weight: u32 },
        SetOrderType { new_order_type: OrderTypeSeed },
    }
    impl<U> From<NoItems<U>> for Full<never::Arg, U> {
        fn from(value: NoItems<U>) -> Self {
            use NoItems as Seed;
            match value {
                Seed::AddJoint => Self::AddJoint,
                Seed::AddBucket => Self::AddBucket,
                Seed::SetFilters { new_filters } => Self::SetFilters { new_filters },
                Seed::SetWeight { new_weight } => Self::SetWeight { new_weight },
                Seed::SetOrderType { new_order_type } => Self::SetOrderType { new_order_type },
            }
        }
    }
    // Prove completeness of `NoItems`
    impl<U> TryFrom<Full<never::Arg, U>> for NoItems<U> {
        type Error = Vec<never::Arg>;

        fn try_from(value: Full<never::Arg, U>) -> Result<Self, Self::Error> {
            use Full as Seed;
            let new = match value {
                Seed::AddBucket => Self::AddBucket,
                Seed::AddJoint => Self::AddJoint,
                Seed::FillBucket { new_contents } => return Err(new_contents),
                Seed::SetFilters { new_filters } => Self::SetFilters { new_filters },
                Seed::SetWeight { new_weight } => Self::SetWeight { new_weight },
                Seed::SetOrderType { new_order_type } => Self::SetOrderType { new_order_type },
            };
            Ok(new)
        }
    }
}

struct NetworkGenerator<S, T, U> {
    _seed_type: std::marker::PhantomData<S>,
    network: Network<T, U>,
}
impl<S, T, U> NetworkGenerator<S, T, U> {
    pub fn finish(self) -> Network<T, U> {
        let Self {
            _seed_type,
            network,
        } = self;
        network
    }
}

mod never {
    #[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub(crate) enum Arg {}
    impl std::str::FromStr for Arg {
        type Err = NeverErr;
        fn from_str(_: &str) -> Result<Self, Self::Err> {
            Err(NeverErr)
        }
    }
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct NeverErr;
    impl std::fmt::Display for NeverErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }
    impl std::error::Error for NeverErr {}
}

impl<'a, S, T, U> arbitrary::Arbitrary<'a> for NetworkGenerator<S, T, U>
where
    S: Into<seed::Full<T, U>> + arbitrary::Arbitrary<'a>,
    T: ArgBounds,
    U: ArgBounds,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        use seed::Full as Seed;

        let mut network = Network::default();

        let mut node_paths = vec![Path::empty()];
        let mut bucket_paths = vec![];
        let mut joint_paths = vec![Path::empty()];

        for _ in 0..u.arbitrary_len::<S>()? {
            let seed: S = u.arbitrary()?;
            let seed = seed.into();
            let path_options = match &seed {
                // only joints
                Seed::AddBucket | Seed::AddJoint => &joint_paths,
                // only buckets
                Seed::FillBucket { .. } => &bucket_paths,
                // any node
                Seed::SetOrderType { .. } => &node_paths,
                // exclude root
                Seed::SetFilters { .. } | Seed::SetWeight { .. } => &node_paths[1..],
            };
            let path = u.choose(path_options)?;

            let len_of_dest = network
                .count_child_nodes_of(path.as_ref())
                .expect("current path should be valid");

            let get_new_path = || {
                let mut new = path.clone();
                new.push(len_of_dest.expect("only add to joint"));

                assert!(!bucket_paths.contains(&new));
                assert!(!joint_paths.contains(&new));
                assert!(!node_paths.contains(&new));

                new
            };

            let path_clone = path.clone();

            // update path lists
            match &seed {
                Seed::AddBucket => {
                    let new_path = get_new_path();

                    bucket_paths.push(new_path.clone());
                    node_paths.push(new_path);
                }
                Seed::AddJoint => {
                    let new_path = get_new_path();

                    joint_paths.push(new_path.clone());
                    node_paths.push(new_path);
                }
                Seed::FillBucket { .. }
                | Seed::SetFilters { .. }
                | Seed::SetWeight { .. }
                | Seed::SetOrderType { .. } => {}
            }

            let cmd = ModifyCmd::from((path_clone, seed));
            let cmd_str = cmd.display_as_cmd().to_string();
            println!("-> {cmd_str}");
            if let Err(e) = network.modify(cmd) {
                panic!("impl Arbitrary for Network should only execute valid commands: {e} \nModifyCmd: {cmd_str}");
            }
        }

        Ok(Self {
            _seed_type: std::marker::PhantomData,
            network,
        })
    }
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        let (lower_bound, _) = S::size_hint(depth);
        (lower_bound, None)
    }
}
