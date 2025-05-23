// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    clap::ArgBounds,
    path::{Path, RemovedSelf},
    ModifyCmd, Network,
};

const DEBUG: bool = false;

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
        DeleteEmpty,
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
                Cmd::DeleteEmpty { path } => (path, Seed::DeleteEmpty),
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
                (path, Seed::DeleteEmpty) => Cmd::DeleteEmpty { path },
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
        DeleteEmpty,
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
                Seed::DeleteEmpty => Self::DeleteEmpty,
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
                Seed::DeleteEmpty => Self::DeleteEmpty,
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

struct ScratchPaths {
    nodes: Vec<Path>,
    buckets: Vec<Path>,
    joints: Vec<Path>,
    emptys: Vec<Path>,
}
impl ScratchPaths {
    fn assert_not_contains(&self, path: &Path) {
        self.all(|label, paths| {
            assert!(
                !paths.contains(path),
                "scratch {label:?} paths should not contain {path}"
            );
        });
    }
    fn add_bucket(&mut self, (parent, bucket): (&Path, Path)) {
        if DEBUG {
            eprintln!("add bucket ({parent}, {bucket})");
        }
        let Self {
            nodes,
            buckets,
            joints: _, // bucket does not affect joints
            emptys,
        } = self;

        // bucket membership
        buckets.push(bucket.clone());
        // node membership
        nodes.push(bucket.clone());

        // new empty
        emptys.push(bucket);
        // parent no longer empty
        emptys.retain(|p| p != parent);
        // FIXME ^^ linear search on many operations using `retain`
        // Only affects tests, but perhaps use BTreeSet to keep sorted output? (already have `Path: Ord`)
    }
    fn add_joint(&mut self, (parent, joint): (&Path, Path)) {
        if DEBUG {
            eprintln!("add joint ({parent}, {joint})");
        }
        let Self {
            nodes,
            buckets: _, // joint does not affect buckets
            joints,
            emptys,
        } = self;

        // joint membership
        joints.push(joint.clone());
        // node membership
        nodes.push(joint.clone());

        // new empty
        emptys.push(joint);
        // parent no longer empty
        emptys.retain(|p| p != parent);
    }
    fn delete(&mut self, node: &Path, parent_now_empty: Option<Path>) {
        self.all_mut(|_label, paths| {
            paths.retain(|p| p != node);
            for path in paths {
                path.modify_for_removed(node.as_ref())
                    .unwrap_or_else(|_: RemovedSelf| {
                        panic!("deleted path {node} should already be removed from the list")
                    });
            }
        });

        if let Some(parent) = parent_now_empty {
            self.emptys.push(parent);
        }
    }
    fn fill_bucket(&mut self, bucket: &Path, empty: bool) {
        let Self {
            nodes: _, // fill does not affect any node membership
            buckets: _,
            joints: _,
            emptys,
        } = self;
        emptys.retain(|p| p != bucket);
        if empty {
            emptys.push(bucket.clone());
        }
    }
    fn all(&self, mut f: impl FnMut(&str, &[Path])) {
        enum Never {}
        self.try_all(|label, elems| {
            f(label, elems);
            Ok(())
        })
        .unwrap_or_else(|n: Never| match n {});
    }
    fn try_all<E>(&self, mut f: impl FnMut(&str, &[Path]) -> Result<(), E>) -> Result<(), E> {
        let Self {
            nodes,
            buckets,
            joints,
            emptys,
        } = self;
        f("nodes", nodes)?;
        f("buckets", buckets)?;
        f("joints", joints)?;
        f("emptys", emptys)?;
        Ok(())
    }
    fn all_mut(&mut self, f: impl Fn(&str, &mut Vec<Path>)) {
        let Self {
            nodes,
            buckets,
            joints,
            emptys,
        } = self;
        f("nodes", nodes);
        f("buckets", buckets);
        f("joints", joints);
        f("emptys", emptys);
    }
}
impl std::fmt::Debug for ScratchPaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ScratchPaths {{")?;
        self.try_all(|label, paths| {
            write!(f, "\t{label}: [ ")?;
            for path in paths {
                write!(f, "{path} ")?;
            }
            writeln!(f, "],")
        })?;
        writeln!(f, "}}")
    }
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

        let mut scratch = ScratchPaths {
            nodes: vec![Path::empty()],
            buckets: vec![],
            joints: vec![Path::empty()],
            emptys: vec![],
        };

        for _ in 0..u.arbitrary_len::<S>()? {
            let seed: S = u.arbitrary()?;
            let seed = seed.into();
            let path_options = match &seed {
                // only joints
                Seed::AddBucket | Seed::AddJoint => &scratch.joints,
                // only buckets
                Seed::FillBucket { .. } => &scratch.buckets,
                // any node
                Seed::SetOrderType { .. } => &scratch.nodes,
                // exclude root
                Seed::SetFilters { .. } | Seed::SetWeight { .. } => &scratch.nodes[1..],
                // only empty nodes
                Seed::DeleteEmpty => &scratch.emptys,
            };
            if path_options.is_empty() {
                // no paths for the chosen seed, retry for the next seed
                continue;
            }
            let path = u.choose(path_options)?;

            let len_of_dest = network
                .count_direct_child_nodes_of(path.as_ref())
                .expect("current path should be valid");

            let get_new_path = || {
                let mut new = path.clone();
                new.push(len_of_dest.expect("only add to joint"));

                scratch.assert_not_contains(&new);

                new
            };

            let path_clone = path.clone();

            // update path lists
            match &seed {
                Seed::AddBucket => {
                    let new_path = get_new_path();

                    scratch.add_bucket((&path_clone, new_path));
                }
                Seed::AddJoint => {
                    let new_path = get_new_path();

                    scratch.add_joint((&path_clone, new_path));
                }
                Seed::DeleteEmpty => {
                    let parent_now_empty =
                        path_clone.as_ref().split_last().and_then(|(last, parent)| {
                            // necessary condition: deleted must be index `0` to be the last one
                            if last == 0 {
                                // verify no siblings remain
                                let new_child_count = network
                                    .count_direct_child_nodes_of(parent)
                                    .expect("parent should be valid path")
                                    .expect("parent should be a joint");
                                (new_child_count == 0).then_some(parent.to_owned())
                            } else {
                                None
                            }
                        });
                    scratch.delete(&path_clone, parent_now_empty);
                }
                Seed::FillBucket { new_contents } => {
                    let empty = new_contents.is_empty();
                    scratch.fill_bucket(&path_clone, empty);
                }
                Seed::SetFilters { .. } | Seed::SetWeight { .. } | Seed::SetOrderType { .. } => {}
            }

            let cmd = ModifyCmd::from((path_clone, seed));
            let cmd_str = cmd.as_ref().display_as_cmd().to_string();
            if DEBUG {
                println!("-> {cmd_str}");
            }
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
