// Copyright 2022 Singularity Data
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;

use paste::paste;

use super::*;
pub use crate::expr::CollectInputRef;
use crate::optimizer::share_parent_counter::ShareParentCounter;
use crate::optimizer::PlanVisitor;
use crate::{for_batch_plan_nodes, for_stream_plan_nodes};

/// The trait for column pruning, only logical plan node will use it, though all plan node impl it.
pub trait ColPrunable {
    /// Transform the plan node to only output the required columns ordered by index number.
    ///
    /// `required_cols` must be a subset of the range `0..self.schema().len()`.
    ///
    /// After calling `prune_col` on the children, their output schema may change, so
    /// the caller may need to transform its [`InputRef`](crate::expr::InputRef) using
    /// [`ColIndexMapping`](crate::utils::ColIndexMapping).
    ///
    /// When implementing this method for a node, it may require its children to produce additional
    /// columns besides `required_cols`. In this case, it may need to insert a
    /// [`LogicalProject`](super::LogicalProject) above to have a correct schema.
    fn prune_col(&self, required_cols: &[usize], ctx: &mut ColumnPruningContext) -> PlanRef;
}

/// Implements [`ColPrunable`] for batch and streaming node.
macro_rules! impl_prune_col {
    ($( { $convention:ident, $name:ident }),*) => {
        paste!{
            $(impl ColPrunable for [<$convention $name>] {
                fn prune_col(&self, _required_cols: &[usize], _ctx: &mut ColumnPruningContext) -> PlanRef {
                    panic!("column pruning is only allowed on logical plan")
                }
            })*
        }
    }
}
for_batch_plan_nodes! { impl_prune_col }
for_stream_plan_nodes! { impl_prune_col }

#[derive(Debug, Clone)]
pub struct ColumnPruningContext {
    share_required_cols_map: HashMap<PlanNodeId, Vec<Vec<usize>>>,
    share_parent_counter: ShareParentCounter,
}

impl ColumnPruningContext {
    pub fn new(root: PlanRef) -> Self {
        let mut share_parent_counter = ShareParentCounter::default();
        share_parent_counter.visit(root.clone());
        Self {
            share_required_cols_map: Default::default(),
            share_parent_counter,
        }
    }

    pub fn get_parent_num(&self, share: &LogicalShare) -> usize {
        self.share_parent_counter.get_parent_num(share)
    }

    pub fn add_required_cols(
        &mut self,
        plan_node_id: PlanNodeId,
        required_cols: Vec<usize>,
    ) -> usize {
        self.share_required_cols_map
            .entry(plan_node_id)
            .and_modify(|e| e.push(required_cols.clone()))
            .or_insert_with(|| vec![required_cols])
            .len()
    }

    pub fn take_required_cols(&mut self, plan_node_id: PlanNodeId) -> Option<Vec<Vec<usize>>> {
        self.share_required_cols_map.remove(&plan_node_id)
    }
}
