use crate::quadrant::Quadrant;

pub struct Address {
    data: Vec<Quadrant>,
}

impl Address {
    pub fn depth(&self) -> usize {
        return self.data.len();
    }

    pub fn at(&self, index: usize) -> Quadrant {
        return self.data[index];
    }

    pub fn has(&self, index: usize) -> bool {
        return index > 0 && index < self.depth();
    }
}

impl From<Vec<Quadrant>> for Address {
    fn from(data: Vec<Quadrant>) -> Self {
        return Self { data };
    }
}

impl From<Address> for Vec<Quadrant> {
    fn from(address: Address) -> Self {
        return address.data;
    }
}
