use quadtree::VisitData;
use state::{BranchState, LeafState};

use crate::engine::{Engine, Error};
use crate::fields::{FieldPass, FieldsComputationData, FieldsState};

// size of downsampled block, in meters. important for getting good performance out of the blur.
const BLOCK_SIZE: f32 = 200.0;

impl Engine {
    pub fn update_fields(&mut self) -> Result<(), Error> {
        // TODO: Pass in more pieces of state once that is necessary. It's not possible to pass all
        // of Engine because it can't be borrowed both mutably and immutably at the same time.
        let mut fold = UpdateFieldsFold::new(FieldsComputationData {
            config: &self.state.config,
            agents: &self.agents,
        });

        fold.run_pass(&mut self.state.qtree, FieldPass::First)?;

        crate::field_blur::perform_field_blur(
            &mut self.state.qtree,
            &self.state.config,
            |f| f.raw_land_value.raw_land_value.value as u8,
            |f, v, data| {
                f.land_value.land_value = crate::fields::WeightedAverage {
                    value: v as f64,
                    count: data.width.pow(2) as usize,
                }
            },
            800.0,
            BLOCK_SIZE,
        )?;

        crate::field_blur::perform_field_blur(
            &mut self.state.qtree,
            &self.state.config,
            |f| f.raw_land_value.raw_construction_cost.value as u8,
            |f, v, data| {
                f.land_value.construction_cost = crate::fields::WeightedAverage {
                    value: v as f64,
                    count: data.width.pow(2) as usize,
                }
            },
            300.0,
            BLOCK_SIZE,
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
        let changed =
            leaf.fields
                .compute_leaf(&leaf.tile, data, &self.field_computation_data, self.pass);
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
