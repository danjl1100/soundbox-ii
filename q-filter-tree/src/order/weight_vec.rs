use std::iter::FromIterator;

use crate::{order, Weight};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct WeightVec<T>(Vec<Weight>, Vec<T>);
impl<T> WeightVec<T> {
    pub fn new() -> Self {
        Self(vec![], vec![])
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<(Weight, &T)> {
        match (self.0.get(index), self.1.get(index)) {
            (Some(&weight), Some(node)) => Some((weight, node)),
            _ => None,
        }
    }
    pub fn weights(&self) -> &[Weight] {
        &self.0
    }
    pub fn elems(&self) -> &[T] {
        &self.1
    }
    pub fn get_elem_mut(&mut self, index: usize) -> Option<&mut T> {
        self.1.get_mut(index)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Weight, &T)> {
        self.weights().iter().zip(self.elems().iter())
    }
    pub fn ref_mut<'order>(&mut self, order: &'order mut order::State) -> RefMut<'_, 'order, T> {
        self.ref_mut_optional(Some(order))
    }
    pub fn ref_mut_optional<'order>(
        &mut self,
        order: Option<&'order mut order::State>,
    ) -> RefMut<'_, 'order, T> {
        RefMut {
            weights: &mut self.0,
            elems: &mut self.1,
            order,
        }
    }
}
impl<T> FromIterator<(Weight, T)> for WeightVec<T> {
    fn from_iter<U: IntoIterator<Item = (Weight, T)>>(iter: U) -> Self {
        let mut weight_vec = WeightVec::new();
        {
            let mut ref_mut = weight_vec.ref_mut_optional(None);
            for item in iter {
                ref_mut.push(item);
            }
        }
        weight_vec
    }
}
pub(crate) struct RefMut<'vec, 'order, T> {
    weights: &'vec mut Vec<Weight>,
    elems: &'vec mut Vec<T>,
    order: Option<&'order mut order::State>,
}
impl<'vec, 'order, T> RefMut<'vec, 'order, T> {
    pub fn set_weight(&mut self, index: usize, new_weight: Weight) -> Result<(), usize> {
        if let Some(weight) = self.weights.get_mut(index) {
            // changed
            *weight = new_weight;
            if let Some(order) = &mut self.order {
                order.notify_changed(Some(index), self.weights);
            }
            Ok(())
        } else {
            Err(index)
        }
    }
    pub fn get_elem_mut(&mut self, index: usize) -> Option<&mut T> {
        // no update needed - weight not changed
        self.elems.get_mut(index)
    }
    pub fn push(&mut self, (weight, item): (Weight, T)) {
        // no updated needed - not removed, nor changed
        self.weights.push(weight);
        self.elems.push(item);
    }
    pub fn remove(&mut self, index: usize) -> Result<(Weight, T), usize> {
        if index < self.weights.len() {
            let removed = (self.weights.remove(index), self.elems.remove(index));
            if let Some(order) = &mut self.order {
                order.notify_removed(index, self.weights);
            }
            Ok(removed)
        } else {
            Err(index)
        }
    }
    pub fn overwrite_weights(&mut self, weights: Vec<Weight>) -> Result<(), (Vec<Weight>, usize)> {
        let orig_len = self.weights.len();
        if weights.len() == orig_len {
            *self.weights = weights;
            if let Some(order) = &mut self.order {
                order.notify_changed(None, self.weights);
            }
            assert_eq!(
                self.weights.len(),
                self.elems.len(),
                "weights and items lists length equal after overwrite_child_weights"
            );
            Ok(())
        } else {
            Err((weights, orig_len))
        }
    }
    pub fn into_elem_ref(
        self,
        index: usize,
    ) -> Result<(RefMutWeight<'vec, 'order>, &'vec mut T), usize> {
        assert_eq!(
            self.weights.len(),
            self.elems.len(),
            "weights and items lists length equal after overwrite_child_weights"
        );
        let Self {
            weights,
            elems,
            order,
        } = self;
        elems
            .get_mut(index)
            .map(move |elem| {
                (
                    RefMutWeight {
                        weights,
                        order,
                        index,
                    },
                    elem,
                )
            })
            .ok_or(index)
    }
}
pub(crate) type RefMutElem<'vec, 'order, T> = (RefMutWeight<'vec, 'order>, &'vec mut T);
pub(crate) struct RefMutWeight<'vec, 'order> {
    // ref_mut: RefMut<'a, 'b, T>,
    weights: &'vec mut Vec<Weight>,
    order: Option<&'order mut order::State>,
    index: usize,
}
impl<'vec, 'order> RefMutWeight<'vec, 'order> {
    pub fn set_weight(&mut self, weight: Weight) {
        let weight_mut = self
            .weights
            .get_mut(self.index)
            .expect("valid index in created WeightVecMutElem");
        *weight_mut = weight;
        if let Some(order) = &mut self.order {
            order.notify_changed(Some(self.index), self.weights);
        }
    }
    pub fn get_weight(&self) -> Weight {
        *self
            .weights
            .get(self.index)
            .expect("valid index in created WeightVecMutElem")
    }
}
