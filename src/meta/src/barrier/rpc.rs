// Copyright 2024 RisingWave Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::future::poll_fn;
use std::task::Poll;
use std::time::Duration;

use anyhow::anyhow;
use fail::fail_point;
use futures::future::try_join_all;
use futures::StreamExt;
use itertools::Itertools;
use risingwave_common::catalog::{DatabaseId, TableId};
use risingwave_common::util::tracing::TracingContext;
use risingwave_meta_model::WorkerId;
use risingwave_pb::common::{ActorInfo, WorkerNode};
use risingwave_pb::meta::PausedReason;
use risingwave_pb::stream_plan::barrier_mutation::Mutation;
use risingwave_pb::stream_plan::{Barrier, BarrierMutation, StreamActor, SubscriptionUpstreamInfo};
use risingwave_pb::stream_service::streaming_control_stream_request::{
    CreatePartialGraphRequest, PbInitRequest, PbInitialPartialGraph, RemovePartialGraphRequest,
};
use risingwave_pb::stream_service::{
    streaming_control_stream_request, streaming_control_stream_response, BarrierCompleteResponse,
    InjectBarrierRequest, StreamingControlStreamRequest,
};
use risingwave_rpc_client::StreamingControlHandle;
use rw_futures_util::pending_on_none;
use thiserror_ext::AsReport;
use tokio::time::{sleep, timeout};
use tokio_retry::strategy::ExponentialBackoff;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::{Command, InflightSubscriptionInfo};
use crate::barrier::context::{GlobalBarrierWorkerContext, GlobalBarrierWorkerContextImpl};
use crate::barrier::info::{BarrierInfo, InflightDatabaseInfo};
use crate::controller::fragment::InflightFragmentInfo;
use crate::manager::MetaSrvEnv;
use crate::{MetaError, MetaResult};

const COLLECT_ERROR_TIMEOUT: Duration = Duration::from_secs(3);

fn to_partial_graph_id(database_id: DatabaseId, job_id: Option<TableId>) -> u64 {
    ((database_id.database_id as u64) << u32::BITS)
        | (job_id
            .map(|table| {
                assert_ne!(table.table_id, u32::MAX);
                table.table_id
            })
            .unwrap_or(u32::MAX) as u64)
}

pub(super) fn from_partial_graph_id(partial_graph_id: u64) -> (DatabaseId, Option<TableId>) {
    let database_id = DatabaseId::new((partial_graph_id >> u32::BITS) as u32);
    let job_id = (partial_graph_id & (u32::MAX as u64)) as u32;
    let job_id = if job_id == u32::MAX {
        None
    } else {
        Some(TableId::new(job_id))
    };
    (database_id, job_id)
}

struct ControlStreamNode {
    worker: WorkerNode,
    handle: StreamingControlHandle,
}

pub(super) struct ControlStreamManager {
    nodes: HashMap<WorkerId, ControlStreamNode>,
    env: MetaSrvEnv,
}

impl ControlStreamManager {
    pub(super) fn new(env: MetaSrvEnv) -> Self {
        Self {
            nodes: Default::default(),
            env,
        }
    }

    pub(super) async fn add_worker(
        &mut self,
        node: WorkerNode,
        initial_subscriptions: impl Iterator<Item = (DatabaseId, &InflightSubscriptionInfo)>,
        context: &impl GlobalBarrierWorkerContext,
    ) {
        let node_id = node.id as WorkerId;
        if self.nodes.contains_key(&node_id) {
            warn!(id = node.id, host = ?node.host, "node already exists");
            return;
        }
        let node_host = node.host.clone().unwrap();
        let mut backoff = ExponentialBackoff::from_millis(100)
            .max_delay(Duration::from_secs(3))
            .factor(5);
        let init_request = Self::collect_init_request(initial_subscriptions);
        const MAX_RETRY: usize = 5;
        for i in 1..=MAX_RETRY {
            match context.new_control_stream(&node, &init_request).await {
                Ok(handle) => {
                    assert!(self
                        .nodes
                        .insert(
                            node_id,
                            ControlStreamNode {
                                worker: node.clone(),
                                handle,
                            }
                        )
                        .is_none());
                    info!(?node_host, "add control stream worker");
                    return;
                }
                Err(e) => {
                    // It may happen that the dns information of newly registered worker node
                    // has not been propagated to the meta node and cause error. Wait for a while and retry
                    let delay = backoff.next().unwrap();
                    error!(attempt = i, backoff_delay = ?delay, err = %e.as_report(), ?node_host, "fail to resolve worker node address");
                    sleep(delay).await;
                }
            }
        }
        error!(?node_host, "fail to create worker node after retry");
    }

    pub(super) async fn reset(
        &mut self,
        initial_subscriptions: impl Iterator<Item = (DatabaseId, &InflightSubscriptionInfo)>,
        nodes: &HashMap<WorkerId, WorkerNode>,
        context: &impl GlobalBarrierWorkerContext,
    ) -> MetaResult<()> {
        let init_request = Self::collect_init_request(initial_subscriptions);
        let init_request = &init_request;
        let nodes = try_join_all(nodes.iter().map(|(worker_id, node)| async move {
            let handle = context.new_control_stream(node, init_request).await?;
            Result::<_, MetaError>::Ok((
                *worker_id,
                ControlStreamNode {
                    worker: node.clone(),
                    handle,
                },
            ))
        }))
        .await?;
        self.nodes.clear();
        for (worker_id, node) in nodes {
            assert!(self.nodes.insert(worker_id, node).is_none());
        }

        Ok(())
    }

    /// Clear all nodes and response streams in the manager.
    pub(super) fn clear(&mut self) {
        *self = Self::new(self.env.clone());
    }

    async fn next_response(
        &mut self,
    ) -> Option<(
        WorkerId,
        MetaResult<streaming_control_stream_response::Response>,
    )> {
        if self.nodes.is_empty() {
            return None;
        }
        let (worker_id, result) = poll_fn(|cx| {
            for (worker_id, node) in &mut self.nodes {
                match node.handle.response_stream.poll_next_unpin(cx) {
                    Poll::Ready(result) => {
                        return Poll::Ready((
                            *worker_id,
                            result
                                .ok_or_else(|| anyhow!("end of stream").into())
                                .and_then(|result| {
                                    result.map_err(Into::<MetaError>::into).and_then(|resp| {
                                        match resp
                                            .response
                                            .ok_or_else(||anyhow!("empty response"))?
                                        {
                                            streaming_control_stream_response::Response::Shutdown(_) => Err(anyhow!(
                                                "worker node {worker_id} is shutting down"
                                            )
                                            .into()),
                                            streaming_control_stream_response::Response::Init(_) => {
                                                // This arm should be unreachable.
                                                Err(anyhow!("get unexpected init response").into())
                                            }
                                            resp => Ok(resp),
                                        }
                                    })
                                }),
                        ));
                    }
                    Poll::Pending => {
                        continue;
                    }
                }
            }
            Poll::Pending
        })
        .await;

        if let Err(err) = &result {
            let node = self
                .nodes
                .remove(&worker_id)
                .expect("should exist when get shutdown resp");
            warn!(node = ?node.worker, err = %err.as_report(), "get error from response stream");
        }

        Some((worker_id, result))
    }

    pub(super) async fn next_collect_barrier_response(
        &mut self,
    ) -> (WorkerId, MetaResult<BarrierCompleteResponse>) {
        use streaming_control_stream_response::Response;

        {
            let (worker_id, result) = pending_on_none(self.next_response()).await;

            (
                worker_id,
                result.map(|resp| match resp {
                    Response::CompleteBarrier(resp) => resp,
                    Response::Shutdown(_) | Response::Init(_) => {
                        unreachable!("should be treated as error")
                    }
                }),
            )
        }
    }

    pub(super) async fn collect_errors(
        &mut self,
        worker_id: WorkerId,
        first_err: MetaError,
    ) -> Vec<(WorkerId, MetaError)> {
        let mut errors = vec![(worker_id, first_err)];
        #[cfg(not(madsim))]
        {
            let _ = timeout(COLLECT_ERROR_TIMEOUT, async {
                while let Some((worker_id, result)) = self.next_response().await {
                    if let Err(e) = result {
                        errors.push((worker_id, e));
                    }
                }
            })
            .await;
        }
        tracing::debug!(?errors, "collected stream errors");
        errors
    }

    fn collect_init_request(
        initial_subscriptions: impl Iterator<Item = (DatabaseId, &InflightSubscriptionInfo)>,
    ) -> PbInitRequest {
        PbInitRequest {
            graphs: initial_subscriptions
                .map(|(database_id, info)| PbInitialPartialGraph {
                    partial_graph_id: to_partial_graph_id(database_id, None),
                    subscriptions: info.into_iter().collect_vec(),
                })
                .collect(),
        }
    }
}

impl ControlStreamManager {
    pub(super) fn inject_command_ctx_barrier(
        &mut self,
        database_id: DatabaseId,
        command: Option<&Command>,
        barrier_info: &BarrierInfo,
        prev_paused_reason: Option<PausedReason>,
        pre_applied_graph_info: &InflightDatabaseInfo,
        applied_graph_info: &InflightDatabaseInfo,
    ) -> MetaResult<HashSet<WorkerId>> {
        let mutation = command.and_then(|c| c.to_mutation(prev_paused_reason));
        let subscriptions_to_add = if let Some(Mutation::Add(add)) = &mutation {
            add.subscriptions_to_add.clone()
        } else {
            vec![]
        };
        let subscriptions_to_remove = if let Some(Mutation::DropSubscriptions(drop)) = &mutation {
            drop.info.clone()
        } else {
            vec![]
        };
        self.inject_barrier(
            database_id,
            None,
            mutation,
            barrier_info,
            pre_applied_graph_info.fragment_infos(),
            applied_graph_info.fragment_infos(),
            command
                .as_ref()
                .map(|command| command.actors_to_create())
                .unwrap_or_default(),
            subscriptions_to_add,
            subscriptions_to_remove,
        )
    }

    pub(super) fn inject_barrier<'a>(
        &mut self,
        database_id: DatabaseId,
        creating_table_id: Option<TableId>,
        mutation: Option<Mutation>,
        barrier_info: &BarrierInfo,
        pre_applied_graph_info: impl IntoIterator<Item = &InflightFragmentInfo>,
        applied_graph_info: impl IntoIterator<Item = &'a InflightFragmentInfo> + 'a,
        mut new_actors: Option<HashMap<WorkerId, Vec<StreamActor>>>,
        subscriptions_to_add: Vec<SubscriptionUpstreamInfo>,
        subscriptions_to_remove: Vec<SubscriptionUpstreamInfo>,
    ) -> MetaResult<HashSet<WorkerId>> {
        fail_point!("inject_barrier_err", |_| risingwave_common::bail!(
            "inject_barrier_err"
        ));

        let partial_graph_id = to_partial_graph_id(database_id, creating_table_id);

        let node_actors = InflightFragmentInfo::actor_ids_to_collect(pre_applied_graph_info);

        for worker_id in node_actors.keys() {
            if !self.nodes.contains_key(worker_id) {
                return Err(anyhow!("unconnected worker node {}", worker_id).into());
            }
        }

        let table_ids_to_sync: HashSet<_> =
            InflightFragmentInfo::existing_table_ids(applied_graph_info)
                .map(|table_id| table_id.table_id)
                .collect();

        let mut node_need_collect = HashSet::new();
        let new_actors_location_to_broadcast = new_actors
            .iter()
            .flatten()
            .flat_map(|(worker_id, actor_infos)| {
                actor_infos.iter().map(|actor_info| ActorInfo {
                    actor_id: actor_info.actor_id,
                    host: self
                        .nodes
                        .get(worker_id)
                        .expect("have checked exist previously")
                        .worker
                        .host
                        .clone(),
                })
            })
            .collect_vec();

        self.nodes
            .iter()
            .try_for_each(|(node_id, node)| {
                let actor_ids_to_collect = node_actors
                    .get(node_id)
                    .map(|actors| actors.iter().cloned())
                    .into_iter()
                    .flatten()
                    .collect();
                {
                    let mutation = mutation.clone();
                    let barrier = Barrier {
                        epoch: Some(risingwave_pb::data::Epoch {
                            curr: barrier_info.curr_epoch.value().0,
                            prev: barrier_info.prev_epoch(),
                        }),
                        mutation: mutation.clone().map(|_| BarrierMutation { mutation }),
                        tracing_context: TracingContext::from_span(barrier_info.curr_epoch.span())
                            .to_protobuf(),
                        kind: barrier_info.kind.to_protobuf() as i32,
                        passed_actors: vec![],
                    };

                    node.handle
                        .request_sender
                        .send(StreamingControlStreamRequest {
                            request: Some(
                                streaming_control_stream_request::Request::InjectBarrier(
                                    InjectBarrierRequest {
                                        request_id: Uuid::new_v4().to_string(),
                                        barrier: Some(barrier),
                                        actor_ids_to_collect,
                                        table_ids_to_sync: table_ids_to_sync
                                            .iter()
                                            .cloned()
                                            .collect(),
                                        partial_graph_id,
                                        broadcast_info: new_actors_location_to_broadcast.clone(),
                                        actors_to_build: new_actors
                                            .as_mut()
                                            .map(|new_actors| new_actors.remove(&(*node_id as _)))
                                            .into_iter()
                                            .flatten()
                                            .flatten()
                                            .collect(),
                                        subscriptions_to_add: subscriptions_to_add.clone(),
                                        subscriptions_to_remove: subscriptions_to_remove.clone(),
                                    },
                                ),
                            ),
                        })
                        .map_err(|_| {
                            MetaError::from(anyhow!(
                                "failed to send request to {} {:?}",
                                node.worker.id,
                                node.worker.host
                            ))
                        })?;

                    node_need_collect.insert(*node_id as WorkerId);
                    Result::<_, MetaError>::Ok(())
                }
            })
            .inspect_err(|e| {
                // Record failure in event log.
                use risingwave_pb::meta::event_log;
                let event = event_log::EventInjectBarrierFail {
                    prev_epoch: barrier_info.prev_epoch(),
                    cur_epoch: barrier_info.curr_epoch.value().0,
                    error: e.to_report_string(),
                };
                self.env
                    .event_log_manager_ref()
                    .add_event_logs(vec![event_log::Event::InjectBarrierFail(event)]);
            })?;
        Ok(node_need_collect)
    }

    pub(super) fn add_partial_graph(
        &mut self,
        database_id: DatabaseId,
        creating_job_id: Option<TableId>,
    ) -> MetaResult<()> {
        let partial_graph_id = to_partial_graph_id(database_id, creating_job_id);
        self.nodes.iter().try_for_each(|(_, node)| {
            node.handle
                .request_sender
                .send(StreamingControlStreamRequest {
                    request: Some(
                        streaming_control_stream_request::Request::CreatePartialGraph(
                            CreatePartialGraphRequest { partial_graph_id },
                        ),
                    ),
                })
                .map_err(|_| anyhow!("failed to add partial graph"))
        })?;
        Ok(())
    }

    pub(super) fn remove_partial_graph(
        &mut self,
        database_id: DatabaseId,
        creating_job_ids: Vec<TableId>,
    ) {
        if creating_job_ids.is_empty() {
            return;
        }
        let partial_graph_ids = creating_job_ids
            .into_iter()
            .map(|job_id| to_partial_graph_id(database_id, Some(job_id)))
            .collect_vec();
        self.nodes.iter().for_each(|(_, node)| {
            if node.handle
                .request_sender
                .send(StreamingControlStreamRequest {
                    request: Some(
                        streaming_control_stream_request::Request::RemovePartialGraph(
                            RemovePartialGraphRequest {
                                partial_graph_ids: partial_graph_ids.clone(),
                            },
                        ),
                    ),
                })
                .is_err()
            {
                warn!(worker_id = node.worker.id,node = ?node.worker.host,"failed to send remove partial graph request");
            }
        })
    }
}

impl GlobalBarrierWorkerContextImpl {
    pub(super) async fn new_control_stream_impl(
        &self,
        node: &WorkerNode,
        init_request: &PbInitRequest,
    ) -> MetaResult<StreamingControlHandle> {
        let handle = self
            .env
            .stream_client_pool()
            .get(node)
            .await?
            .start_streaming_control(init_request.clone())
            .await?;
        Ok(handle)
    }
}

pub(super) fn merge_node_rpc_errors<E: Error + Send + Sync + 'static>(
    message: &str,
    errors: impl IntoIterator<Item = (WorkerId, E)>,
) -> MetaError {
    use std::error::request_value;
    use std::fmt::Write;

    use risingwave_common::error::tonic::extra::Score;

    let errors = errors.into_iter().collect_vec();

    if errors.is_empty() {
        return anyhow!(message.to_owned()).into();
    }

    // Create the error from the single error.
    let single_error = |(worker_id, e)| {
        anyhow::Error::from(e)
            .context(format!("{message}, in worker node {worker_id}"))
            .into()
    };

    if errors.len() == 1 {
        return single_error(errors.into_iter().next().unwrap());
    }

    // Find the error with the highest score.
    let max_score = errors
        .iter()
        .filter_map(|(_, e)| request_value::<Score>(e))
        .max();

    if let Some(max_score) = max_score {
        let mut errors = errors;
        let max_scored = errors
            .extract_if(|(_, e)| request_value::<Score>(e) == Some(max_score))
            .next()
            .unwrap();

        return single_error(max_scored);
    }

    // The errors do not have scores, so simply concatenate them.
    let concat: String = errors
        .into_iter()
        .fold(format!("{message}: "), |mut s, (w, e)| {
            write!(&mut s, " in worker node {}, {};", w, e.as_report()).unwrap();
            s
        });
    anyhow!(concat).into()
}

#[cfg(test)]
mod tests {
    use risingwave_common::catalog::{DatabaseId, TableId};

    use crate::barrier::rpc::{from_partial_graph_id, to_partial_graph_id};

    #[test]
    fn test_partial_graph_id_convert() {
        fn test_convert(database_id: u32, job_id: Option<u32>) {
            let database_id = DatabaseId::new(database_id);
            let job_id = job_id.map(TableId::new);
            assert_eq!(
                (database_id, job_id),
                from_partial_graph_id(to_partial_graph_id(database_id, job_id))
            );
        }
        for database_id in [0, 1, 2, u32::MAX - 1, u32::MAX >> 1] {
            for job_id in [
                Some(0),
                Some(1),
                Some(2),
                None,
                Some(u32::MAX >> 1),
                Some(u32::MAX - 1),
            ] {
                test_convert(database_id, job_id);
            }
        }
    }
}
