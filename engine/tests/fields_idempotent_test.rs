use std::collections::HashMap;
use std::path::PathBuf;

use engine::{Engine, FieldsState};
use quadtree::{Address, VisitData};
use state::{BranchState, LeafState};

#[test]
fn fields_idempotent_test() {
    // Make sure the fields don't change if you run the update again.
    // NOTE: we can't just compare the serialized values because we don't serialize fields.

    let mut engine = Engine::load_file(&PathBuf::from("maps/sf.json")).unwrap();

    engine.update_fields().unwrap();
    let mut first_collector = CollectFieldsVisitor::default();
    engine.state.qtree.visit(&mut first_collector);

    engine.update_fields().unwrap();
    let mut second_collector = CollectFieldsVisitor::default();
    engine.state.qtree.visit(&mut second_collector);

    assert!(first_collector == second_collector);
}

#[derive(PartialEq, Default)]
struct CollectFieldsVisitor {
    fields: HashMap<quadtree::Address, FieldsState>,
}

impl quadtree::Visitor<BranchState<FieldsState>, LeafState<FieldsState>, anyhow::Error>
    for CollectFieldsVisitor
{
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<bool, anyhow::Error> {
        assert_eq!(
            self.fields.insert(data.address, branch.fields.clone()),
            None
        );
        Ok(true)
    }

    fn visit_leaf(
        &mut self,
        leaf: &LeafState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), anyhow::Error> {
        assert_eq!(self.fields.insert(data.address, leaf.fields.clone()), None);
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
