use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use snafu::{OptionExt, Snafu};
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::domain::nar::actor::{NarActorEffect, NarActorState};
use crate::domain::nar::model::{Nar, NarInfoData, NarInfoQueryOutcome, NarState};
use crate::domain::nar::port::NarInfoProvider;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug)]
pub enum NarMessage {
    ResolveNarInfo(OneshotSender<NarResolveResponse>),
    Evict,
}

#[derive(Debug)]
pub struct NarResolveResponse {
    pub result: Result<NarInfoData, ResolveNarInfoError>,
    pub effects: Vec<NarActorEffect>,
}

#[derive(Snafu, Debug)]
#[non_exhaustive]
pub enum ResolveNarInfoError {
    #[snafu(display("could not fetch narinfo"))]
    Fetch,
}

pub struct NarActor {
    messages: MpscReceiver<NarMessage>,
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_info_provider: Arc<dyn NarInfoProvider>,
}

impl NarActor {
    pub fn new(
        messages: MpscReceiver<NarMessage>,
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_info_provider: Arc<dyn NarInfoProvider>,
    ) -> Self {
        Self {
            messages,
            substituter_availability_index,
            nar_info_provider,
        }
    }

    pub fn run(mut self, nar: Nar) {
        let mut state = NarActorState::new(nar);
        tokio::spawn(async move {
            while let Some(message) = self.messages.recv().await {
                if matches!(message, NarMessage::Evict) {
                    break;
                }
                state = self.handle_message(state, message).await;
            }
        });
    }

    async fn handle_message(&mut self, state: NarActorState, message: NarMessage) -> NarActorState {
        match message {
            NarMessage::ResolveNarInfo(reply) => {
                self.handle_message_resolve_nar_info(state, reply).await
            }
            NarMessage::Evict => state,
        }
    }

    pub async fn handle_message_resolve_nar_info(
        &mut self,
        state: NarActorState,
        reply: OneshotSender<NarResolveResponse>,
    ) -> NarActorState {
        match state.inner().state() {
            NarState::Resolved { nar_info, .. } => {
                let _ = reply.send(NarResolveResponse {
                    result: Ok(nar_info.clone()),
                    effects: vec![],
                });
                state
            }
            NarState::Empty => {
                let substituters = self.substituter_availability_index.query_all();
                let outcomes_fut = substituters
                    .iter()
                    .map(|meta| self.start_nar_info_query(&state, meta));
                let outcomes = futures::future::join_all(outcomes_fut).await;

                let (effects, state) =
                    NarActorState::on_all_outcomes_acquired(state, outcomes, &substituters);

                let nar_state = state.inner().state();
                let result = nar_state.clone().into_nar_info().context(FetchSnafu);
                let _ = reply.send(NarResolveResponse { result, effects });
                state
            }
        }
    }

    async fn start_nar_info_query(
        &self,
        state: &NarActorState,
        meta: &SubstituterMeta,
    ) -> AnyhowResult<NarInfoQueryOutcome> {
        let provider = Arc::clone(&self.nar_info_provider);
        let url = state.inner().hash().on_substituter(meta);
        provider.provide_nar_info(&url).await
    }
}
