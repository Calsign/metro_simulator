use once_cell::unsync::OnceCell;
use rand::distributions::weighted::WeightedError;
use rand_distr::weighted_alias::WeightedAliasIndex;

use quadtree::VisitData;
use state::{BranchState, LeafState};

use crate::engine::{Engine, Error};
use crate::fields::{FieldPass, FieldsComputationData, FieldsState, WeightedAverage};

// size of downsampled block, in meters. important for getting good performance out of the blur.
const BLOCK_SIZE: f32 = 200.0;

#[derive(Debug, Clone, Default)]
pub(crate) struct BlurredField {
    buffer: Vec<u8>,
    dim: u64,
    downsample: u64,
    distr: OnceCell<Option<WeightedAliasIndex<u64>>>,
}

impl BlurredField {
    pub fn sample<R: rand::Rng>(
        &self,
        rng: &mut R,
        qtree: &quadtree::Quadtree<BranchState<FieldsState>, LeafState<FieldsState>>,
    ) -> Option<quadtree::Address> {
        use rand::distributions::Distribution;

        let distr = self.distr.get_or_init(|| {
            // NOTE: it's important to convert to something bigger than u8 so that we don't have overflow.
            // WeightedAliasIndex includes this requirement:
            //   For any weight w: w < 0 or w > max where max = W::MAX / weights.len().
            // But I also had issues with WeightedIndex.
            let weights: Vec<u64> = self.buffer.iter().map(|x| *x as u64).collect();
            match WeightedAliasIndex::new(weights) {
                Ok(distr) => Some(distr),
                Err(WeightedError::AllWeightsZero) => {
                    // this is... fine, I guess?
                    eprintln!("all workplace demand weights zero");
                    None
                }
                // other errors are not fine
                Err(err) => {
                    panic!("Error creating weighted index distribution: {}", err)
                }
            }
        });

        distr.as_ref().map(|distr| {
            let index = distr.sample(rng);
            let (mut x, mut y) = index_to_coords(index, self.dim);

            // scale up to the full dimensions
            x *= self.downsample;
            y *= self.downsample;

            // each block in the blurred buffer is several minimal tiles in the qtree
            // so pick a tile within the block at random
            // TODO: it could be more efficient to construct a Uniform and store it?
            x += rng.gen_range(0..self.downsample);
            y += rng.gen_range(0..self.downsample);

            qtree.get_address(x, y).unwrap()
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BlurredFields {
    pub land_value: BlurredField,
    pub construction_cost: BlurredField,
    pub workplace_demand: BlurredField,
}

impl Engine {
    fn perform_blur_weighted_average<G, S>(
        field: &mut BlurredField,
        qtree: &mut quadtree::Quadtree<BranchState<FieldsState>, LeafState<FieldsState>>,
        config: &state::Config,
        getter: G,
        setter: S,
        radius: f32,
        block_size: f32,
        scale: f64,
    ) -> Result<(), Error>
    where
        G: Fn(&FieldsState) -> &WeightedAverage,
        S: Fn(&mut FieldsState) -> &mut WeightedAverage,
    {
        perform_field_blur(
            field,
            qtree,
            config,
            |f| (getter(f).value * scale) as u8,
            |f, v, data| {
                *setter(f) = WeightedAverage {
                    value: v as f64 / scale,
                    count: data.width.pow(2) as usize,
                }
            },
            radius,
            block_size,
        )
    }

    pub fn update_fields(&mut self) -> Result<(), Error> {
        // TODO: Pass in more pieces of state once that is necessary. It's not possible to pass all
        // of Engine because it can't be borrowed both mutably and immutably at the same time.
        let mut fold = UpdateFieldsFold::new(FieldsComputationData {
            config: &self.state.config,
            agents: &self.agents,
        });

        fold.run_pass(&mut self.state.qtree, FieldPass::First)?;

        Self::perform_blur_weighted_average(
            &mut self.blurred_fields.land_value,
            &mut self.state.qtree,
            &self.state.config,
            |f| &f.raw_land_value.raw_land_value,
            |f| &mut f.land_value.land_value,
            800.0,
            BLOCK_SIZE,
            1.0,
        )?;

        Self::perform_blur_weighted_average(
            &mut self.blurred_fields.construction_cost,
            &mut self.state.qtree,
            &self.state.config,
            |f| &f.raw_land_value.raw_construction_cost,
            |f| &mut f.land_value.construction_cost,
            300.0,
            BLOCK_SIZE,
            1.0,
        )?;

        Self::perform_blur_weighted_average(
            &mut self.blurred_fields.workplace_demand,
            &mut self.state.qtree,
            &self.state.config,
            |f| &f.raw_demand.raw_workplace_demand,
            |f| &mut f.demand.workplace_demand,
            600.0,
            BLOCK_SIZE,
            30.0,
        )?;

        // second pass runs after blurs
        fold.run_pass(&mut self.state.qtree, FieldPass::Second)?;

        Ok(())
    }
}

struct UpdateFieldsFold<'a, 'b> {
    field_computation_data: FieldsComputationData<'a, 'b>,
    pass: FieldPass,
}

impl<'a, 'b> UpdateFieldsFold<'a, 'b> {
    fn new(field_computation_data: FieldsComputationData<'a, 'b>) -> Self {
        Self {
            field_computation_data,
            pass: FieldPass::First,
        }
    }

    fn run_pass(
        &mut self,
        qtree: &mut quadtree::Quadtree<BranchState<FieldsState>, LeafState<FieldsState>>,
        pass: FieldPass,
    ) -> Result<(), Error> {
        self.pass = pass;
        qtree.fold_mut(self)?;
        Ok(())
    }
}

impl<'a, 'b>
    quadtree::MutFold<BranchState<FieldsState>, LeafState<FieldsState>, (bool, FieldsState), Error>
    for UpdateFieldsFold<'a, 'b>
{
    fn fold_leaf(
        &mut self,
        leaf: &mut LeafState<FieldsState>,
        data: &VisitData,
    ) -> Result<(bool, FieldsState), Error> {
        let changed = leaf.fields.compute_leaf(
            &leaf.tile,
            leaf.creation_time,
            data,
            &self.field_computation_data,
            self.pass,
        );
        Ok((changed, leaf.fields.clone()))
    }

    fn fold_branch(
        &mut self,
        branch: &mut BranchState<FieldsState>,
        children: &quadtree::QuadMap<(bool, FieldsState)>,
        data: &VisitData,
    ) -> Result<(bool, FieldsState), Error> {
        let changed = children.values().iter().any(|(c, _)| *c);
        if changed {
            // only recompute branch if at least one of the children changed
            let fields = children.clone().map_into(&|(_, f)| f);
            branch
                .fields
                .compute_branch(&fields, data, &self.field_computation_data, self.pass);
        }
        Ok((changed, branch.fields.clone()))
    }
}

pub(crate) fn coords_to_index(x: u64, y: u64, dim: u64) -> usize {
    (x + y * dim) as usize
}

pub(crate) fn index_to_coords(index: usize, dim: u64) -> (u64, u64) {
    (index as u64 % dim, index as u64 / dim)
}

/**
 * Blur a field value.
 *
 * Important: this function converts input values to u8. So make sure that the inputs work with
 * that.
 *
 * Arguments:
 *   qtree: the qtree with fields to blur
 *   config: the map config
 *   getter: getter for accessing the raw field value from FieldsState
 *   setter: setter for setting the output field value in FieldsState
 *   radius: the blurring radius, in meters
 *   block_size: the width of downsampled block, in meters
 */
fn perform_field_blur<G, S>(
    field: &mut BlurredField,
    qtree: &mut quadtree::Quadtree<BranchState<FieldsState>, LeafState<FieldsState>>,
    config: &state::Config,
    getter: G,
    setter: S,
    radius: f32,
    block_size: f32,
) -> Result<(), Error>
where
    G: Fn(&FieldsState) -> u8,
    S: Fn(&mut FieldsState, u8, &VisitData),
{
    // round to power of two
    let downsample = 2_u64.pow((block_size / config.min_tile_size as f32).log2().floor() as u32);
    let sigma = radius / config.min_tile_size as f32 / downsample as f32;
    // from here on out, don't use radius and block_size, just use downsample and sigma

    let dim = qtree.width() / downsample;
    let len = dim.pow(2) as usize;
    if field.buffer.len() != len {
        field.buffer = vec![0; len];
    } else {
        field.buffer.fill(0);
    }

    let mut input_visitor = BlurInputVisitor {
        buffer: &mut field.buffer,
        getter,
        dim,
        downsample,
    };
    qtree.visit(&mut input_visitor)?;

    fastblur::gaussian_blur_asymmetric_single_channel(
        &mut field.buffer,
        dim as usize,
        dim as usize,
        sigma,
        sigma,
    );

    let mut output_visitor = BlurOutputVisitor {
        buffer: &mut field.buffer,
        setter,
        dim,
        downsample,
    };
    qtree.visit_mut(&mut output_visitor)?;

    field.dim = dim;
    field.downsample = downsample;
    // reset the distribution so that we lazily compute it again as needed
    field.distr.take();

    Ok(())
}

struct BlurInputVisitor<'a, G>
where
    G: Fn(&FieldsState) -> u8,
{
    buffer: &'a mut Vec<u8>,
    getter: G,
    dim: u64,
    downsample: u64,
}

impl<'a, G> BlurInputVisitor<'a, G>
where
    G: Fn(&FieldsState) -> u8,
{
    fn apply(&mut self, fields: &FieldsState, data: &VisitData) {
        let value = (self.getter)(fields);

        if data.width > self.downsample {
            // apply value to entire area, which spans multiple blocks in the buffer
            for y in data.y..(data.y + data.width) {
                // this relies on the data being column-major
                let start_index =
                    coords_to_index(data.x / self.downsample, y / self.downsample, self.dim);
                let end_index = coords_to_index(
                    (data.x + data.width) / self.downsample,
                    y / self.downsample,
                    self.dim,
                );
                self.buffer[start_index..end_index].fill(value);
            }
        } else {
            // scale down each value by the downsample amount because we combine multiple values into one block
            let scaled_value = value / (self.downsample - data.width + 1).pow(2) as u8;
            let index =
                coords_to_index(data.x / self.downsample, data.y / self.downsample, self.dim);
            self.buffer[index] += value;
        }
    }
}

impl<'a, G> quadtree::Visitor<BranchState<FieldsState>, LeafState<FieldsState>, Error>
    for BlurInputVisitor<'a, G>
where
    G: Fn(&FieldsState) -> u8,
{
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<bool, Error> {
        // NOTE: I experimented with applying an entire branch to avoid descending into each leaf
        // when downsampling is applied, but it did not yield any significant change in performance.
        Ok(true)
    }

    fn visit_leaf(&mut self, leaf: &LeafState<FieldsState>, data: &VisitData) -> Result<(), Error> {
        self.apply(&leaf.fields, data);
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), Error> {
        Ok(())
    }
}

struct BlurOutputVisitor<'a, S>
where
    S: Fn(&mut FieldsState, u8, &VisitData),
{
    buffer: &'a Vec<u8>,
    setter: S,
    dim: u64,
    downsample: u64,
}

impl<'a, S> quadtree::MutVisitor<BranchState<FieldsState>, LeafState<FieldsState>, Error>
    for BlurOutputVisitor<'a, S>
where
    S: Fn(&mut FieldsState, u8, &VisitData),
{
    fn visit_branch_pre(
        &mut self,
        branch: &mut BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<bool, Error> {
        Ok(true)
    }

    fn visit_leaf(
        &mut self,
        leaf: &mut LeafState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), Error> {
        // sample the block at the center of the buffer
        let (x, y) = data.center();
        let index = coords_to_index(x / self.downsample, y / self.downsample, self.dim);
        let value = self.buffer[index];
        (self.setter)(&mut leaf.fields, value, data);
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &mut BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), Error> {
        Ok(())
    }
}
