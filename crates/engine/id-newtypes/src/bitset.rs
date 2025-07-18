#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct BitSet<Id> {
    inner: fixedbitset::FixedBitSet,
    _phantom: std::marker::PhantomData<Id>,
}

impl<Id> Default for BitSet<Id> {
    fn default() -> Self {
        Self {
            inner: fixedbitset::FixedBitSet::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<Id> BitSet<Id>
where
    usize: From<Id>,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            inner: fixedbitset::FixedBitSet::with_capacity(n),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn put(&mut self, id: Id) -> bool {
        self.inner.put(usize::from(id))
    }

    pub fn set(&mut self, id: Id, value: bool) {
        self.inner.set(usize::from(id), value)
    }

    pub fn push(&mut self, value: bool) {
        self.inner.grow(self.inner.len() + 1);
        self.inner.set(self.inner.len() - 1, value);
    }

    pub fn grow(&mut self, n: usize) {
        self.inner.grow(n)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn zeroes(&self) -> impl Iterator<Item = Id> + '_
    where
        Id: From<usize>,
    {
        self.inner.zeroes().map(|ix| Id::from(ix))
    }

    pub fn ones(&self) -> impl Iterator<Item = Id> + '_
    where
        Id: From<usize>,
    {
        self.inner.ones().map(|ix| Id::from(ix))
    }

    pub fn set_all(&mut self, value: bool) {
        self.inner.set_range(.., value);
    }
}

impl<Id> std::ops::Index<Id> for BitSet<Id>
where
    usize: From<Id>,
{
    type Output = bool;
    fn index(&self, index: Id) -> &Self::Output {
        &self.inner[usize::from(index)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitset() {
        let mut bitset = BitSet::<usize>::with_capacity(129);
        for i in 0..129 {
            assert!(!bitset[i]);
        }

        bitset.set(100, true);
        assert!(!bitset[99]);
        assert!(bitset[100]);
        assert!(!bitset[101]);
    }
}
