use quadtree::VisitData;
use state::{BranchState, LeafState};

use crate::engine::{Engine, Error};
use crate::fields::FieldsState;

fn coords_to_index(x: u64, y: u64, dim: u64) -> usize {
    (x + y * dim) as usize
}

fn index_to_coords(index: usize, dim: u64) -> (u64, u64) {
    (index as u64 % dim, index as u64 / dim)
}

/**
 * Blur a field value.
 *
 * Arguments:
 *   qtree: the qtree with fields to blur
 *   config: the map config
 *   getter: getter for accessing the raw field value from FieldsState
 *   setter: setter for setting the output field value in FieldsState
 *   radius: the blurring radius, in meters
 *   block_size: the width of downsampled block, in meters
 */
pub fn perform_field_blur<G, S>(
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
    // TODO: Don't reallocate the buffer every time!
    // Flamegraphs have not yet shown this to be a performance issue.
    let mut buffer: Vec<u8> = vec![0; dim.pow(2) as usize];

    let mut input_visitor = BlurInputVisitor {
        buffer: &mut buffer,
        getter,
        dim,
        downsample,
    };
    qtree.visit(&mut input_visitor)?;

    fastblur::gaussian_blur_asymmetric_single_channel(
        &mut buffer,
        dim as usize,
        dim as usize,
        sigma,
        sigma,
    );

    let mut output_visitor = BlurOutputVisitor {
        buffer: &buffer,
        setter,
        dim,
        downsample,
    };
    qtree.visit_mut(&mut output_visitor)?;

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
