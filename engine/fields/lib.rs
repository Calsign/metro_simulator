struct ComputeLeafData<'a, 'b> {
    tile: &'a tiles::Tile,
    data: &'b quadtree::VisitData,
}

struct ComputeBranchData<'a, 'b> {
    fields: &'a quadtree::QuadMap<FieldsState>,
    data: &'b quadtree::VisitData,
}

trait Field: std::fmt::Debug + Default + Clone {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self>;

    fn compute_branch(branch: ComputeBranchData) -> Option<Self>;
}

#[derive(Debug, Default, Clone)]
pub struct Population {
    pub total: usize,
    pub density: f64,
}

impl Population {
    fn from_total(total: usize, data: &quadtree::VisitData) -> Self {
        let density = total as f64 / (data.width * data.width) as f64;
        Self { total, density }
    }
}

impl Field for Population {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let total = match leaf.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { density, .. }) => *density,
            _ => 0,
        };
        Some(Self::from_total(total, leaf.data))
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        let total = branch
            .fields
            .values()
            .iter()
            .map(|f| f.population.total)
            .sum();
        Some(Self::from_total(total, branch.data))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Employment {
    pub total: usize,
    pub density: f64,
}

impl Employment {
    fn from_total(total: usize, data: &quadtree::VisitData) -> Self {
        let density = total as f64 / (data.width * data.width) as f64;
        Self { total, density }
    }
}

impl Field for Employment {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        let total = match leaf.tile {
            tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, .. }) => *density,
            _ => 0,
        };
        Some(Self::from_total(total, leaf.data))
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        let total = branch
            .fields
            .values()
            .iter()
            .map(|f| f.employment.total)
            .sum();
        Some(Self::from_total(total, branch.data))
    }
}

#[derive(Debug, Default, Clone)]
pub struct LandValue {
    pub value: f64,
}

impl Field for LandValue {
    fn compute_leaf(leaf: ComputeLeafData) -> Option<Self> {
        Some(Self { value: 0.0 })
    }

    fn compute_branch(branch: ComputeBranchData) -> Option<Self> {
        Some(Self { value: 0.0 })
    }
}

// TODO: write a procedural macro to make this less painful
#[derive(Debug, Default, Clone)]
pub struct FieldsState {
    pub population: Population,
    pub employment: Employment,
    pub land_value: LandValue,
}

impl FieldsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compute_leaf(&mut self, tile: &tiles::Tile, data: &quadtree::VisitData) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_leaf(ComputeLeafData { tile, data }) {
                    Some(val) => {
                        self.$name = val;
                        changed = true;
                    }
                    None => (),
                }
            }};
        }

        each_field!(Population, population);
        each_field!(Employment, employment);
        each_field!(LandValue, land_value);

        changed
    }

    pub fn compute_branch(
        &mut self,
        fields: &quadtree::QuadMap<FieldsState>,
        data: &quadtree::VisitData,
    ) -> bool {
        let mut changed = false;

        macro_rules! each_field {
            ($field:ty, $name:ident) => {{
                match <$field>::compute_branch(ComputeBranchData { fields, data }) {
                    Some(val) => {
                        self.$name = val;
                        changed = true;
                    }
                    None => (),
                }
            }};
        }

        each_field!(Population, population);
        each_field!(Employment, employment);
        each_field!(LandValue, land_value);

        changed
    }
}
