use rkyv::{Archive, Deserialize, Serialize};

#[derive(Eq, PartialEq, Debug, Clone, Archive, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Vec1<T>(Vec<T>);

impl<T> Vec1<T> {
    pub fn new(t: T) -> Self {
        let mut v = Vec::with_capacity(1);
        v.push(t);
        Vec1(v)
    }

    pub fn push(&mut self, t: T) {
        self.0.push(t)
    }

    pub fn append(&mut self, other: &mut Vec<T>) {
        self.0.append(other)
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    pub fn as_vec(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> IntoIterator for Vec1<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T> TryFrom<Vec<T>> for Vec1<T> {
    type Error = ();

    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.len() == 0 {
            Err(())
        } else {
            Ok(Vec1(value))
        }
    }
}

#[macro_export]
macro_rules! vec1 {
    () => (
        compile_error!("Vec1 needs at least 1 element")
    );
    ($first:expr $(, $item:expr)* , ) => (
        $crate::vec1!($first $(, $item)*)
    );
    ($first:expr $(, $item:expr)* ) => ({
        #[allow(unused_mut)]
        let mut tmp = $crate::Vec1::new($first);
        $(tmp.push($item);)*
        tmp
    });
}

pub use vec1;
