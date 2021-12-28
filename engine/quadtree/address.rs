use crate::quadrant::Quadrant;

#[derive(Debug, PartialEq, Eq, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct Address {
    data: Vec<Quadrant>,
}

impl Address {
    pub fn depth(&self) -> usize {
        self.data.len()
    }

    pub fn at(&self, index: usize) -> Quadrant {
        self.data[index]
    }

    pub fn has(&self, index: usize) -> bool {
        index > 0 && index < self.depth()
    }

    pub fn try_from(address: &Vec<u8>) -> Option<Self> {
        let mut vec = Vec::new();
        for index in address.iter() {
            vec.push(Quadrant::try_from(*index)?);
        }
        Some(Address::from(vec))
    }

    pub fn to_vec(self) -> Vec<u8> {
        let mut vec = Vec::new();
        for quad in self.data.iter() {
            vec.push(u8::from(quad));
        }
        vec
    }

    pub fn child(&self, quadrant: Quadrant) -> Self {
        let mut address = self.data.clone();
        address.push(quadrant);
        return address.into();
    }
}

impl From<Vec<Quadrant>> for Address {
    fn from(data: Vec<Quadrant>) -> Self {
        Self { data }
    }
}

impl From<Address> for Vec<Quadrant> {
    fn from(address: Address) -> Self {
        address.data
    }
}

impl From<Address> for Vec<u8> {
    fn from(address: Address) -> Self {
        address.to_vec()
    }
}
